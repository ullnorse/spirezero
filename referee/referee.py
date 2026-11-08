"""Referee harness: drive the real game headless (via sts2-cli's JSON protocol)
and dump the full combat state after every action.

    referee.py run <seed> <actions.json>     -> JSONL trace on stdout
    referee.py serve                         -> one game process for many seeds: reads `<seed>\t<actions.json>\t<trace.jsonl>`
                                                lines on stdin, writes each trace and answers `ok` or `fail <reason>`
    referee.py serve --socket PATH           -> the same as a daemon on a Unix socket, one client at a time (see `cmd_client`)
    referee.py client                        -> `serve` on stdin/stdout, through the slot's daemon if tools/slot.sh set
                                                $SPIREZERO_SLOT_SOCKET, else in this process
    referee.py find-seed <encounter> [--start N] [--limit N] -> first matching seed
    referee.py where                         -> the referee build directory in use (STS2_REFEREE or `current`)

`run` and `serve` go through the trace store (trace_cache.py): an input seen before is answered from it without a game.

Actions file (JSON object); `"mode": "run"` plays a whole run instead, see `cmd_run_mode`:
    {"character": "Necrobinder", "ascension": 0,   // any of Ironclad, Silent, Defect, Regent, Necrobinder
     "act1": "OVERGROWTH",                     // run mode only: or "UNDERDOCKS"
     "act_maps": false,                        // run mode only: step 0 also dumps every act's map (`run.act_maps`)
     "real_shop_prices": false,                // run mode only, each opt-in: take the real-game (TestMode off) branch of
     "real_cauldron": false, "real_calling_bell": false,  // the shop potion price roll / Cauldron / CallingBell rewards
     "encounter": "FUZZY_WURM_CRAWLER_WEAK",   // omit to play the seed's natural first fight
     "neow": 0,                                // natural mode: Neow option to pick
     "loadout": {"deck": ["STRIKE_NECROBINDER", "BODYGUARD+1", "DEFEND_NECROBINDER:VIGOROUS*3"],
                 "relics": ["BOUND_PHYLACTERY"], "potions": ["FIRE_POTION"], "hp": 40, "max_hp": 66},
                 // optional, forced mode only; a deck entry is ID[+UPGRADES][:ENCHANTMENT[*AMOUNT]] (amount 1)
     "actions": [{"action": "play_card", "args": {"card_index": 0, "target_index": 0}},
                 {"action": "end_turn"}]}
"""

import hashlib
import io
import json
import os
import signal
import socket
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))  # `python3 -I` does not add the script's directory
import trace_cache  # noqa: E402
DOTNET = Path(os.environ.get("DOTNET_ROOT", Path.home() / ".dotnet"))

# Pinned run configuration (everything beyond the seed that the run depends on).
CHARACTER = "Necrobinder"
ASCENSION = 0


def resolve_build():
    """The referee build to use, resolved once per process to its immutable `builds/<hash>` directory.

    `STS2_REFEREE` (a build directory or a `current` symlink), else the main checkout's
    `referee/builds/current`, else this checkout's. A later `setup.sh` only re-points the symlink, so a
    process that already resolved it keeps its build.
    """
    candidates = []
    if os.environ.get("STS2_REFEREE"):
        candidates.append(Path(os.environ["STS2_REFEREE"]))
    else:
        common = subprocess.run(["git", "-C", str(HERE), "rev-parse", "--path-format=absolute", "--git-common-dir"],
                                capture_output=True, text=True).stdout.strip()
        if common:
            candidates.append(Path(common).parent / "referee" / "builds" / "current")
        candidates.append(HERE / "builds" / "current")
    for c in candidates:
        if (c / "sts2-cli" / "src" / "Sts2Headless" / "bin" / "Debug" / "net9.0" / "Sts2Headless.dll").exists():
            return c.resolve()
    sys.exit(f"referee not built (looked at {', '.join(map(str, candidates))}); run referee/setup.sh")


BUILD = resolve_build()
VENDOR = BUILD / "sts2-cli"
DLL = VENDOR / "src" / "Sts2Headless" / "bin" / "Debug" / "net9.0" / "Sts2Headless.dll"


# Opt-in real-game branches (referee/patcher, docs/specs/test-mode.md): actions-file key -> environment variable.
REAL_VARS = {"real_shop_prices": "STS2_REAL_SHOP", "real_cauldron": "STS2_REAL_CAULDRON", "real_calling_bell": "STS2_REAL_BELL"}


class GameDied(SystemExit):
    """The game process is gone or out of step (a crash, a closed pipe, a reset that could not restore it): not the
    job's fault, so `Server.job` replays the job on a fresh process."""


class Game:
    """One headless game process. To reuse it for another seed, `reset` it first (tracer/Reset.cs)."""

    def __init__(self, act1=None, real=()):
        env = dict(os.environ, DOTNET_ROOT=str(DOTNET), PATH=f"{DOTNET}:{os.environ['PATH']}")
        env.pop("STS2_ACT1", None)
        for var in REAL_VARS.values():
            env.pop(var, None)
        for key in real:
            env[REAL_VARS[key]] = "1"
        if act1 is not None:
            env["STS2_ACT1"] = act1
        env.setdefault("STS2_GAME_DIR", "")
        trace_cache.note("game")
        self.proc = subprocess.Popen(
            ["dotnet", str(DLL)],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=None if os.environ.get("REFEREE_DEBUG") else subprocess.DEVNULL,
            text=True, env=env, cwd=VENDOR,
        )
        self._read()  # "ready"

    def _read(self):
        # The engine prints a few non-JSON notices on stdout; skip them.
        while True:
            line = self.proc.stdout.readline()
            if not line:
                raise GameDied("game process exited unexpectedly")
            if line.startswith("{"):
                try:
                    return json.loads(line)
                except json.JSONDecodeError as e:
                    raise GameDied(f"game wrote bad JSON: {e}") from None

    def send(self, cmd):
        try:
            self.proc.stdin.write(json.dumps(cmd, separators=(",", ":")) + "\n")
            self.proc.stdin.flush()
        except OSError as e:
            raise GameDied(f"game process exited unexpectedly ({e})") from None
        return self._read()

    def reset(self, act1, real):
        """End the run and set the per-run environment (tracer/Reset.cs), like a fresh `Game(act1, real)`."""
        env = {REAL_VARS[key]: "1" for key in real}
        if act1 is not None:
            env["STS2_ACT1"] = act1
        r = self.send({"cmd": "reset", "env": env})
        if r.get("type") != "reset":
            raise GameDied(f"reset failed: {r}")
        if r.get("stale_turn_loop"):
            raise GameDied("reset: the last combat's turn loop is still running")

    def close(self):
        try:
            self.send({"cmd": "quit"})
        except SystemExit:
            pass
        try:
            self.proc.wait(timeout=30)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            self.proc.wait()


def start(game, seed, spec):
    encounter = spec.get("encounter")
    # A forced fight never reads the act map, so the game skips its pruning (referee/patcher, docs/referee-perf.md).
    r = game.send({"cmd": "start_run", "character": spec.get("character", CHARACTER),
                   "ascension": spec.get("ascension", ASCENSION), "seed": seed, "forced_fight": bool(encounter)})
    if r.get("type") == "error":
        sys.exit(f"start_run failed: {r}")
    if spec.get("loadout"):
        if not encounter:
            sys.exit("loadout needs a forced encounter")
        r = game.send({"cmd": "set_loadout", **spec["loadout"]})
        if r.get("type") == "error":
            sys.exit(f"loadout failed: {r.get('message')}")
    if encounter:
        r = game.send({"cmd": "enter_room", "type": "combat", "encounter": encounter})
        # A start-of-combat prompt (GamblingChip's discard) is the fight's first decision: step 0 shows
        # it and the scripted actions answer it.
        reached = ("combat_play", "card_select")
    else:
        r = natural_first_fight(game, r, spec.get("neow", 0))
        reached = ("combat_play",)
    if r.get("type") == "error" or r.get("decision") not in reached:
        sys.exit(f"could not reach combat: {json.dumps(r)[:400]}")
    return r


def natural_first_fight(game, r, neow):
    """Neow -> (rewards/continue) -> first map node, taking the first option each time."""
    for _ in range(20):
        d = r.get("decision")
        if d == "combat_play":
            return r
        if d == "event_choice":
            r = game.send({"cmd": "action", "action": "choose_option", "args": {"option_index": neow}})
            neow = 0  # later event pages: take the first option
        elif d == "card_select":
            r = game.send({"cmd": "action", "action": "select_cards", "args": {"indices": "0"}})
        elif d == "card_reward":
            r = game.send({"cmd": "action", "action": "skip_card_reward"})
        elif d == "map_select":
            c = r["choices"][0]
            r = game.send({"cmd": "action", "action": "select_map_node",
                           "args": {"col": c["col"], "row": c["row"]}})
        else:
            r = game.send({"cmd": "action", "action": "proceed"})
    return r


def snapshot(game, step, action, resp, first=False):
    state = game.send({"cmd": "dump_state"})
    if not first:
        state.pop("first_encounters", None)
    line = {"step": step, "action": action, "decision": resp.get("decision", resp.get("type")), "state": state}
    if resp.get("victory"):  # a `game_over` that won the run (tracer/RunEnd.cs); a death has no key
        line["victory"] = True
    if resp.get("type") == "error":
        line["error"] = resp.get("message")
    return line


OUT = sys.stdout  # where `emit` writes the trace; `serve` points it at each job's file


def emit(line):
    OUT.write(json.dumps(line, separators=(",", ":"), sort_keys=False) + "\n")


def run_dump(game, with_map, act_maps=False):
    cmd = {"cmd": "dump_run", "map": with_map}
    if act_maps:
        cmd["act_maps"] = True
    run = game.send(cmd)
    if run.get("type") == "error":
        sys.exit(f"dump_run failed: {run}")
    return run


def cmd_run_mode(game, seed, spec):
    """Whole run from the seed: Neow, map, rooms, rewards, shop, rest, events and combat.

    Actions are sts2-cli's own (`select_map_node {col,row}`, `choose_option {option_index}`, `select_cards {indices}`,
    `select_card_reward {card_index}`, `skip_card_reward`, `buy_card {card_index}`, `buy_relic {relic_index}`,
    `buy_potion {potion_index}`, `remove_card`, `leave_room`, `proceed` and the combat ones), applied in order.
    `run_loadout {...}` (tracer/RunLoadout.cs) and `enter_event {event}` (sts2-cli's `enter_room` with type `event`: the event
    room is entered from wherever the run is, like `RunManager.EnterRoom`) are our own, not sts2-cli actions.
    Every trace line is `{step, action, decision, state, run}`: `state` is the combat dump (`dump_state`) and
    `run` is `dump_run`, which carries the full map only on the first line and on the first line of each new act.
    """
    character = spec.get("character", CHARACTER)
    ascension = spec.get("ascension", ASCENSION)
    resp = game.send({"cmd": "start_run", "character": character, "ascension": ascension, "seed": seed})
    if resp.get("type") == "error":
        sys.exit(f"start_run failed: {resp}")
    run = run_dump(game, True, spec.get("act_maps", False))
    line = snapshot(game, 0, {"action": "start", "seed": seed, "character": character,
                              "ascension": ascension, "mode": "run"}, resp, first=True)
    line["run"] = run
    emit(line)
    for i, a in enumerate(spec["actions"], 1):
        if a["action"] == "run_loadout":  # not an sts2-cli action: our own command, see tracer/RunLoadout.cs
            resp = game.send({"cmd": "run_loadout", **a.get("args", {})})
        elif a["action"] == "enter_event":  # not an action either: sts2-cli's `enter_room` command for any event
            resp = game.send({"cmd": "enter_room", "type": "event", "event": a.get("args", {}).get("event", "")})
        else:
            resp = game.send({"cmd": "action", "action": a["action"], "args": a.get("args", {})})
        line = snapshot(game, i, a, resp)
        new_run = run_dump(game, False)
        if new_run["act"] != run["act"]:
            new_run = run_dump(game, True)
        run = new_run
        line["run"] = run
        emit(line)
        if "error" in line:
            sys.exit(f"step {i} failed: {line['error']}")


def run_config(path):
    """The actions file and the process configuration it needs: (spec, act1, real)."""
    spec = json.loads(Path(path).read_text())
    if isinstance(spec, list):
        spec = {"actions": spec}
    act1 = spec.get("act1", "OVERGROWTH").upper()
    if act1 not in ("OVERGROWTH", "UNDERDOCKS"):
        sys.exit(f"unknown act1 '{act1}' (OVERGROWTH or UNDERDOCKS)")
    if act1 == "UNDERDOCKS" and spec.get("mode") != "run":
        sys.exit('"act1" is for run mode; force an Underdocks fight with "encounter" instead')
    real = [key for key in REAL_VARS if spec.get(key)]
    if real and spec.get("mode") != "run":
        sys.exit(f"{', '.join(real)} are for run mode (the real-game branches matter on shop and relic rooms)")
    return spec, act1.lower(), real


def cmd_run(seed, path):
    global OUT
    spec, act1, real = run_config(path)
    key = trace_cache.key(seed, spec, BUILD, DOTNET)
    if (hit := trace_cache.lookup(key)) is not None:
        sys.stdout.write(hit[0])
        if hit[1] != "ok":
            sys.exit(hit[1].removeprefix("fail "))
        return
    OUT = sys.stdout if key is None else io.StringIO()
    status = None  # stays None if interrupted: nothing is stored
    game = Game(act1, real)
    try:
        play(game, seed, spec)
        status = "ok"
    except SystemExit as e:
        status = f"fail {e.code}"
        raise
    finally:
        game.close()
        if key is not None:
            sys.stdout.write(OUT.getvalue())
            if status is not None:
                trace_cache.store(key, OUT.getvalue(), status)


# A game process is replaced after this many seeds: it grows by about 0.25 MB per run.
RECYCLE_RUNS = 500
# Identity of a serving daemon: its referee build and this script. A client with another one replaces the daemon.
# The environment the game reads (trace_cache.env_inputs) is part of it: the daemon's game inherits its spawner's.
IDENT = hashlib.sha256(str(BUILD).encode() + Path(__file__).read_bytes()
                       + json.dumps(trace_cache.env_inputs()).encode()).hexdigest()[:16]


class Server:
    """Many seeds, one game process (see `run_config` for the per-run setup and tracer/Reset.cs for the reset).

    A job that fails (a rejected action, a dead process) still leaves its trace up to the failure; the process is
    then replaced, so a later job never starts from a game in an unknown state. A job whose process died
    (`GameDied`), or that failed on a reused process, is replayed once on a fresh one, whose result is the answer: a
    job never fails because of the process's history. `REFEREE_NO_RETRY=1` turns the replay off (stress_reset.py,
    which looks for exactly those differences).
    """

    def __init__(self):
        self.game = None
        self.runs = 0
        self.retry = not os.environ.get("REFEREE_NO_RETRY")

    def job(self, line):
        """One `<seed>\t<actions.json>\t<trace.jsonl>` job; returns `ok` or `fail <reason>`."""
        seed, path, out = line.rstrip("\n").split("\t")
        reused = self.game is not None
        status, died = self.attempt(seed, path, out)
        if self.retry and (died or (status != "ok" and reused)):
            status, _ = self.attempt(seed, path, out)
        return status

    def attempt(self, seed, path, out):
        """One try of a job: (status, whether the game process died)."""
        global OUT
        status, died = "ok", False
        with open(out, "w") as OUT:
            try:
                spec, act1, real = run_config(path)
                if self.game is None:
                    self.game = Game()
                    self.runs = 0
                self.game.reset(act1, real)
                self.runs += 1
                play(self.game, seed, spec)
            except GameDied as e:
                status, died = f"fail {e.code}", True
            except SystemExit as e:
                status = f"fail {e.code}"
            except Exception as e:  # noqa: BLE001 a harness bug must not take the daemon down
                status, died = f"fail referee error: {e!r}", True
        OUT = sys.stdout
        if status != "ok" or self.runs >= RECYCLE_RUNS:
            self.kill()
        return status.replace("\n", " "), died

    def kill(self):
        if self.game is not None:
            if self.game.proc.poll() is None:
                self.game.proc.kill()
            self.game.proc.wait()
            for f in (self.game.proc.stdin, self.game.proc.stdout):
                try:
                    f.close()
                except OSError:
                    pass
            self.game = None

    def close(self):
        if self.game is not None:
            self.game.close()
            self.game = None


def cached_job(line, run):
    """A `<seed>\t<actions.json>\t<trace.jsonl>` job answered from the trace store (trace_cache.py), else by
    `run(line)`, whose result is stored. Only `cmd_serve` and `cmd_client` call it, so a hit never reaches a game or
    the slot daemon."""
    seed, path, out = line.rstrip("\n").split("\t")
    try:
        spec = run_config(path)[0]
    except SystemExit:
        return run(line)  # a bad actions file: the job reports it
    return trace_cache.answer(trace_cache.key(seed, spec, BUILD, DOTNET), Path(out), lambda: run(line))


def cmd_serve():
    server = Server()
    try:
        for line in sys.stdin:
            sys.stdout.write(cached_job(line, server.job) + "\n")
            sys.stdout.flush()
    finally:
        server.close()


class Terminated(BaseException):
    """SIGTERM in the daemon: not a job failure (`Server.job` catches SystemExit), so it ends the daemon cleanly."""


def terminate(*_):
    raise Terminated


def cmd_serve_socket(path, idle):
    """`serve` as a daemon on a Unix socket: one client at a time, `hello <IDENT>` on connect, the jobs, `quit` ends it.

    Exits after `idle` seconds without a client. The socket's owner (the holder of its tools/slot.sh slot) starts it.
    """
    signal.signal(signal.SIGTERM, terminate)
    server = Server()
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    try:
        Path(path).unlink(missing_ok=True)
        sock.bind(path)
        ino = os.stat(path).st_ino
        sock.listen(1)
        sock.settimeout(idle)
        while True:
            try:
                conn, _ = sock.accept()
            except TimeoutError:
                return
            conn.settimeout(None)
            with conn, conn.makefile("r") as rf, conn.makefile("w") as wf:
                try:
                    wf.write(f"hello {IDENT}\n")
                    wf.flush()
                    for line in rf:
                        if line.strip() == "quit":
                            return
                        wf.write(server.job(line) + "\n")
                        wf.flush()
                except OSError:  # the client went away (Server.job never raises); the next one resets the game anyway
                    pass
    finally:
        sock.close()
        try:  # unless a replacement daemon already bound its own socket there
            if os.stat(path).st_ino == ino:
                Path(path).unlink()
        except (FileNotFoundError, UnboundLocalError):
            pass
        server.close()


def connect(path):
    """A connection to the slot daemon at `path` running this IDENT, starting (or replacing) it as needed.

    A daemon that closes the connection before its hello (it reached its idle timeout as we connected) or that is
    another IDENT (an older layout put every build on one socket) is replaced. Gives up after 3 daemons that never
    answered."""
    spawns = 0
    spawned = deadline = None
    while True:
        conn = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        try:
            conn.connect(path)
            rf, wf = conn.makefile("r"), conn.makefile("w")
            hello = rf.readline().split()
            if hello == ["hello", IDENT]:
                return conn, rf, wf
            if hello:
                wf.write("quit\n")
                wf.flush()
                rf.read()
        except OSError:
            pass
        conn.close()
        if spawned is not None and spawned.poll() is None and time.monotonic() < deadline:
            time.sleep(0.05)  # still starting
            continue
        if spawns == 3:
            sys.exit(f"could not start the referee daemon at {path}")
        if spawned is not None and spawned.poll() is None:
            spawned.kill()
        Path(path).unlink(missing_ok=True)
        env = dict(os.environ, STS2_REFEREE=str(BUILD))
        spawned = subprocess.Popen([sys.executable, "-I", __file__, "serve", "--socket", path,
                                    "--idle", os.environ.get("REFEREE_IDLE", "300")],
                                   stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                                   stderr=None if os.environ.get("REFEREE_DEBUG") else subprocess.DEVNULL,
                                   env=env, start_new_session=True)
        spawns += 1
        deadline = time.monotonic() + 60


def slot_socket(slot):
    """This IDENT's daemon socket for the tools/slot.sh slot socket `slot`: one per build and script version, so
    checkouts with other builds sharing the slots never replace each other's daemon."""
    return f"{slot.removesuffix('.sock')}.{IDENT}.sock"


def cmd_client():
    """`serve` for one script (referee/client.sh). With $SPIREZERO_SLOT_SOCKET (set by tools/slot.sh while it holds the
    slot) the jobs go to that slot's daemon, so the game's start-up is paid once per slot rather than once per script; a
    job whose daemon dies is retried once on a new one, and then in this process."""
    path = os.environ.get("SPIREZERO_SLOT_SOCKET") or os.environ.get("SPIREBOT_SLOT_SOCKET")
    if not path:
        cmd_serve()
        return
    path = slot_socket(path)
    conn = rf = wf = None

    def send(line):
        nonlocal conn, rf, wf
        for _ in range(2):
            try:
                if conn is None:  # connected lazily: a script whose jobs all hit the trace store never reaches the daemon
                    conn, rf, wf = connect(path)
                wf.write(line)
                wf.flush()
                status = rf.readline()
            except (OSError, SystemExit):
                status = ""
            if status:
                return status.rstrip("\n")
            for f in (wf, rf, conn):  # the daemon is gone: drop what is buffered for it
                try:
                    if f is not None:
                        f.close()
                except OSError:
                    pass
            conn = rf = wf = None
        local = Server()  # the daemon died twice: a fresh process of our own
        try:
            return local.job(line)
        finally:
            local.close()

    for line in sys.stdin:
        # The daemon's working directory is not ours.
        seed, *files = line.rstrip("\n").split("\t")
        line = "\t".join([seed, *(os.path.abspath(f) for f in files)]) + "\n"
        sys.stdout.write(cached_job(line, send) + "\n")
        sys.stdout.flush()
    if conn is not None:
        conn.close()


def play(game, seed, spec):
    """One seed's trace on `game`, which has just started or been reset for this spec."""
    if spec.get("mode") == "run":
        cmd_run_mode(game, seed, spec)
        return
    resp = start(game, seed, spec)
    emit(snapshot(game, 0, {"action": "start", "seed": seed,
                            "character": spec.get("character", CHARACTER),
                            "ascension": spec.get("ascension", ASCENSION),
                            "encounter": spec.get("encounter")}, resp, first=True))
    for i, a in enumerate(spec["actions"], 1):
        resp = game.send({"cmd": "action", "action": a["action"], "args": a.get("args", {})})
        line = snapshot(game, i, a, resp)
        emit(line)
        if "error" in line:
            sys.exit(f"step {i} failed: {line['error']}")


def first_encounters(seed):
    game = Game()
    try:
        game.send({"cmd": "start_run", "character": CHARACTER, "ascension": ASCENSION, "seed": seed})
        return game.send({"cmd": "dump_state"})
    finally:
        game.close()


def cmd_find_seed(encounter, start_n=0, limit=200):
    for n in range(start_n, start_n + limit):
        seed = f"T3S{n}"
        s = first_encounters(seed)
        print(f"{seed} act={s['act']} first={s['first_encounters'][0]}", file=sys.stderr)
        if s["first_encounters"][0] == encounter:
            print(seed)
            return
    sys.exit(f"no seed found in {limit} tries")


def main(argv):
    if len(argv) >= 4 and argv[1] == "run":
        cmd_run(argv[2], argv[3])
    elif len(argv) >= 4 and argv[1] == "serve" and argv[2] == "--socket":
        opts = dict(zip(argv[2::2], argv[3::2]))
        try:
            cmd_serve_socket(opts["--socket"], float(opts.get("--idle", 300)))
        except Terminated:
            pass
    elif len(argv) >= 2 and argv[1] == "serve":
        cmd_serve()
    elif len(argv) >= 2 and argv[1] == "client":
        cmd_client()
    elif len(argv) >= 2 and argv[1] == "where":
        print(BUILD)  # the build directory this process resolved, for scripts that must pin it
    elif len(argv) >= 3 and argv[1] == "find-seed":
        opts = dict(zip(argv[3::2], argv[4::2]))
        cmd_find_seed(argv[2], int(opts.get("--start", 0)), int(opts.get("--limit", 200)))
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    try:
        main(sys.argv)
    except BrokenPipeError:  # the reader (e.g. `head`) went away; the game is already closed
        os._exit(0)
