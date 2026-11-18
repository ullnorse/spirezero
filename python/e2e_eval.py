"""The end-to-end Necrobinder evaluation (`docs/plans/necro-e2e.md` §6, WP6).

    python python/e2e_eval.py run CKPT --weights raw|ema0.996|ema0.999 --panel DEV-1000 [--out F]
    python python/e2e_eval.py bot --panel DEV-1000 [--out F]        # the search bot, a CPU job
    python python/e2e_eval.py summary F
    python python/e2e_eval.py compare A B                           # paired, A minus B
    python python/e2e_eval.py replay F                              # every win, in the engine
    python python/e2e_eval.py gate K1|K2|K3|K4 FILE...              # PASS or FAIL

**Panels** (§6; A10 Necrobinder, seeds `RUN<s>`): `SMOKE` (23,000,000+, `--n`, default 1,000),
`DEV-1000`, `DEV-2000` (24,000,000+), `TEST` (25,000,000, 4,000; sealed).

**Result file** (JSONL, byte-identical on a repeat run): line 1 is `{"meta": ...}` (the checkpoint
sha256, the weights, the panel, the code commit and the env options); then one line per run in
seed order: `seed, win, floor, end, bosses, boss_acts, kf, nan, hash, death, fights, fight_acts` and, for a win, `actions`
(base64 of the uint16 action indices; the replay input). `end` is `won`, `died`, `capped` (a turn,
decision or engine-action cap), `unsupported` or `stuck`; `kf` lists the floor of each boss kill;
`death` is the encounter id of the fight that ended a died run (null for a win or a death outside
a fight), `fights` the encounter ids of every fight entered and `fight_acts` their acts;
`hash` is the first 16 hex digits of the sha256 of the run's action list. A run that stops
(`--stop-after` or a kill) resumes from the last whole batch: results are written a batch at a
time, batches are fixed seed chunks, and greedy play is deterministic, so a resumed file equals an
uninterrupted one.

**Sealed TEST:** `run` prints only counts and the sha256 of the result file; a second TEST run of
the same checkpoint and weights is refused unless `--i-know` (the reads are kept in the registry,
`--registry`); `summary`, `compare` and `gate` refuse a TEST file unless `--unseal`.

**Act-2 boss reach** (K2): a run reached it when its total floor is at least `ACT2_BOSS_FLOOR`
(the act's start point plus `ActModel.BaseNumberOfRooms` plus the boss: Overgrowth 1 + 15 + 1 = 17,
Hive 1 + 14 + 1 = 16, so the act-2 boss is floor 33; `act_shape` in `sts2-core/src/run/map.rs`).

The library must be built first: `cargo build --profile ffi -p sts2-ffi`.
"""

import argparse
import base64
import hashlib
import json
import math
import os
import pathlib
import subprocess
import sys
import time
from fractions import Fraction

import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

ROOT = pathlib.Path(__file__).resolve().parents[1]
ACT2_BOSS_FLOOR = 17 + 16
ASCENSION = 10
CHARACTER = "Necrobinder"
Z = 1.959963984540054

PANELS = {  # name -> (first seed, runs)
    "DEV-1000": (24_000_000, 1000),
    "DEV-2000": (24_000_000, 2000),
    "TEST": (25_000_000, 4000),
}
SMOKE_FIRST = 23_000_000
SMOKE_MAX = 1_000_000
CAP_ENDS = ("capped", "fight_capped", "decision_capped")
DEFAULT_REGISTRY = pathlib.Path.home() / "spirebot/target/e2e-eval/test_reads.jsonl"
BOT_BIN = pathlib.Path.home() / "spirebot/target/release/sts2-diff"


def panel_seeds(panel, n=None):
    """`(first seed, runs)` of a panel; `SMOKE` takes `n`."""
    if panel == "SMOKE":
        n = 1000 if n is None else n
        if not 0 < n <= SMOKE_MAX:
            raise SystemExit(f"SMOKE has up to {SMOKE_MAX} seeds")
        return SMOKE_FIRST, n
    if panel not in PANELS:
        raise SystemExit(f"unknown panel {panel!r}: SMOKE, {', '.join(PANELS)}")
    first, count = PANELS[panel]
    if n is not None and n != count:
        raise SystemExit(f"{panel} has exactly {count} seeds")
    return first, count


# ---- statistics ----


def wilson(k, n, z=Z):
    """The Wilson score interval of `k` successes in `n`."""
    if n == 0:
        return 0.0, 1.0
    p = k / n
    d = 1 + z * z / n
    centre = (p + z * z / (2 * n)) / d
    half = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return max(0.0, centre - half), min(1.0, centre + half)


def newcombe_paired(a, b, c, d, z=Z):
    """Newcombe's (1998, method 10) CI for the difference of two paired proportions.

    `a` runs won by both, `b` by the first only, `c` by the second only, `d` by neither;
    the difference is (a+b)/n - (a+c)/n. Returns `(diff, low, high)`.
    """
    n = a + b + c + d
    if n == 0:
        return 0.0, -1.0, 1.0
    p1, p2 = (a + b) / n, (a + c) / n
    l1, u1 = wilson(a + b, n, z)
    l2, u2 = wilson(a + c, n, z)
    margins = (a + b) * (c + d) * (a + c) * (b + d)
    if margins == 0:
        phi = 0.0
    else:
        diff = a * d - b * c
        corr = diff - n / 2 if diff > n / 2 else (0 if diff >= 0 else diff)
        phi = corr / math.sqrt(margins)
    lo_a, hi_b = p1 - l1, u2 - p2
    hi_a, lo_b = u1 - p1, p2 - l2
    delta = p1 - p2
    low = delta - math.sqrt(max(0.0, lo_a**2 + hi_b**2 - 2 * phi * lo_a * hi_b))
    high = delta + math.sqrt(max(0.0, hi_a**2 + lo_b**2 - 2 * phi * hi_a * lo_b))
    return delta, max(-1.0, low), min(1.0, high)


def mcnemar_exact(b, c):
    """The two-sided exact McNemar p: twice the binomial(b+c, 1/2) tail at min(b, c), capped at 1."""
    m = b + c
    if m == 0:
        return 1.0
    tail = sum(math.comb(m, i) for i in range(min(b, c) + 1))
    return float(min(Fraction(1), Fraction(2 * tail, 2**m)))


def paired_mean(xs, ys, z=Z):
    """Mean of x - y with a normal-approximation CI on the paired differences."""
    n = len(xs)
    d = np.asarray(xs, dtype=np.float64) - np.asarray(ys, dtype=np.float64)
    mean = float(d.mean()) if n else 0.0
    if n < 2:
        return mean, mean, mean
    se = float(d.std(ddof=1)) / math.sqrt(n)
    return mean, mean - z * se, mean + z * se


# ---- result files ----


def dumps(obj):
    return json.dumps(obj, sort_keys=True, separators=(",", ":"))


def load_results(path):
    """`(meta, {seed: run})` of a result file; a torn last line is dropped."""
    meta, runs = None, {}
    with open(path, "rb") as f:
        for line in f:
            if not line.endswith(b"\n"):
                break
            obj = json.loads(line)
            if "meta" in obj:
                meta = obj["meta"]
            else:
                runs[obj["seed"]] = obj
    if meta is None:
        raise SystemExit(f"{path}: no meta line")
    return meta, runs


def file_sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def code_commit():
    def git(*args):
        return subprocess.run(["git", "-C", str(ROOT), *args], capture_output=True, text=True)

    head = git("rev-parse", "HEAD").stdout.strip() or "unknown"
    dirty = git("diff", "--quiet", "HEAD", "--", "python", "crates").returncode != 0
    return head + ("+dirty" if dirty else "")


def action_hash(actions):
    return hashlib.sha256(np.asarray(actions, dtype="<u2").tobytes()).hexdigest()[:16]


def pack(actions):
    return base64.b64encode(np.asarray(actions, dtype="<u2").tobytes()).decode()


def unpack(text):
    return np.frombuffer(base64.b64decode(text), dtype="<u2")


# Run observation header (`crates/sts2-rl/src/run_obs.rs`): act index, total floor, the room's
# type (RoomType index + 1 in a combat room, else 0) and its encounter or event (vocab id).
H_ACT, H_FLOOR, H_ROOM, H_MODEL = 2, 7, 26, 28
_ENCOUNTERS = None


def encounter_names():
    """Encounter vocab id -> name (`crates/sts2-rl/vocab/encounters.txt`: the line number is the
    id, and the file only grows, so this stays right for an older frozen library)."""
    global _ENCOUNTERS
    if _ENCOUNTERS is None:
        _ENCOUNTERS = (ROOT / "crates/sts2-rl/vocab/encounters.txt").read_text().splitlines()
    return _ENCOUNTERS


def fight_of(obs):
    """`(floor, encounter id, act index)` of the combat room an obs row is in, else None."""
    if int(obs[H_ROOM]) == 0:
        return None
    return int(obs[H_FLOOR]), int(obs[H_MODEL]) | int(obs[H_MODEL + 1]) << 8, int(obs[H_ACT])


def reached_act2_boss(run):
    return run["floor"] >= ACT2_BOSS_FLOOR or len(run["kf"]) >= 2


def a9_est(run):
    """RLN14, descriptive only: whether the run would have won A9. A10 is A9 plus a second act-3
    boss, so a run that won or killed the first act-3 boss (bit 2 of `boss_acts`) counts, as in
    spire-mentor's `a10_score.py` (their `reached_second_boss`). An estimate, not a measurement:
    the net is ascension-conditioned (theirs read about 5pp under a true A9 sweep)."""
    return bool(run["win"]) or bool(run["boss_acts"] >> 2 & 1)


def sealed(meta, unseal):
    if meta["panel"] == "TEST" and not unseal:
        raise SystemExit("TEST is sealed: pass --unseal for the one final read")


# ---- the greedy panel runner ----


def load_policy(path, weights, device):
    """The checkpoint's net with `weights` (raw, or the EMA of that decay) loaded."""
    import torch

    import train_e2e as T

    state = torch.load(path, map_location="cpu", weights_only=False)
    cfg = T.Config.from_dict(state["config"])
    net = T.net_of(cfg)
    net.load_net(state["net"])
    net.env_opts = dict(discard_potion=cfg.discard_potion, plan_depth=cfg.plan_depth)  # RLN32
    if getattr(cfg, "no_full_heal", False):  # RLN47
        net.env_opts["no_full_heal"] = True
    if weights != "raw":
        if not weights.startswith("ema"):
            raise SystemExit(f"weights: raw, ema0.996 or ema0.999, not {weights!r}")
        decay = float(weights[3:])
        matches = [i for i, d in enumerate(cfg.ema) if abs(d - decay) < 1e-12]
        if not matches:
            raise SystemExit(f"{weights}: the checkpoint has EMAs {list(cfg.ema)}")
        with torch.no_grad():
            for p, t in zip(net.parameters(), state["emas"][matches[0]]):
                p.copy_(t)
    return net.to(device).eval()


def play_batch(net, seeds, device, threads):
    """Greedy runs of `seeds` (one env each); a list of run records, in seed order."""
    import torch

    from sts2 import E2E, HOLD, Sts2RunEnvs
    from sts2.env import RUN_ENDS
    from sts2.e2e import encode as E

    n = len(seeds)
    v2 = bool(getattr(net, "obs_v2", False))
    full = bool(getattr(net, "obs_full", False))
    events = bool(getattr(net, "obs_events", False))
    env = Sts2RunEnvs(n, seeds=np.asarray(seeds, dtype=np.uint64), character=CHARACTER,
                      ascension=ASCENSION, combat="agent", threads=threads,
                      **(dict(obs_v2=True) if v2 else {}), **(dict(obs_full=True) if full else {}),
                      **(dict(obs_events=True) if events else {}),
                      **getattr(net, "env_opts", {}), **E2E)
    E.check_layout(env.layout, v2=v2, full=full, events=events)
    cap = E2E["decision_cap"] + 8
    log = np.zeros((cap, n), dtype=np.uint16)
    steps = np.zeros(n, dtype=np.int64)
    finished = np.zeros(n, dtype=bool)
    floors_cum = np.zeros(n, dtype=np.int64)
    kf = [[] for _ in range(n)]
    boss_acts = np.zeros(n, dtype=np.int64)
    nan = np.zeros(n, dtype=np.int64)
    fights = [[] for _ in range(n)]  # (floor, encounter id, act) of each fight entered
    out = [None] * n
    names = encounter_names()

    def enter(i, fight):
        if fight is not None and (not fights[i] or fights[i][-1] != fight):
            fights[i].append(fight)
    try:
        while not finished.all():
            active = np.nonzero(~finished)[0]
            for i in active:
                enter(i, fight_of(env.obs[i]))
            idx = torch.from_numpy(active)
            obs = torch.from_numpy(env.obs[active]).to(device)
            masks = torch.from_numpy(env.masks.view(np.int64)[active]).to(device)
            with torch.no_grad():
                tok = E.encode(obs, masks, v2=v2, full=full, events=events,
                               discard=bool(getattr(net, "env_opts", {}).get("discard_potion")))
                with torch.autocast(device.type, dtype=torch.bfloat16,
                                    enabled=device.type == "cuda"):
                    logits, _, _ = net.policy(tok)
                logits = logits.float()
                bad = torch.isnan(logits).any(-1)
                action = logits.nan_to_num(nan=float("-inf")).argmax(-1)
            nan[active] += bad.cpu().numpy()
            action = action.cpu().numpy()
            log[steps[active], active] = action
            steps[active] += 1
            step_actions = np.full(n, HOLD, dtype=np.uint16)
            step_actions[active] = action
            env.step(step_actions)
            ev = env.events
            floors_cum[active] += ev[active, 0]
            for i in active[ev[active, 1] > 0]:
                kf[i].append(int(floors_cum[i]))
            boss_acts[active] |= ev[active, 2]
            for i in active[env.done[active].astype(bool)]:
                finished[i] = True
                end = RUN_ENDS[env.end[i]]
                last = fight_of(env.final_obs[i])  # the fight a death came in, else None
                enter(i, last)
                kind = ("won" if env.won[i] else "died" if end == "died" else
                        "capped" if end in CAP_ENDS else end)
                actions = log[: steps[i], i]
                out[i] = {
                    "seed": int(seeds[i]), "win": int(env.won[i]), "floor": int(env.floor[i]),
                    "end": kind, "bosses": len(kf[i]), "boss_acts": int(boss_acts[i]),
                    "kf": kf[i], "nan": int(nan[i]), "hash": action_hash(actions),
                    "death": names[last[1]] if end == "died" and last else None,
                    "fights": [names[e] for _, e, _ in fights[i]],
                    "fight_acts": [a + 1 for _, _, a in fights[i]],
                }
                if env.won[i]:
                    out[i]["actions"] = pack(actions)
    finally:
        env.close()
    return out


def env_options():
    from sts2 import E2E

    return dict(E2E, character=CHARACTER, ascension=ASCENSION, combat="agent", priv=False)


def make_meta(kind, panel, first, count, **extra):
    return {"kind": kind, "panel": panel, "first": first, "count": count,
            "commit": code_commit(), "env": env_options(), **extra}


def start_file(path, meta, resume_ok=True):
    """Opens `path` for appending: a fresh file gets the meta line; an existing one must have the
    same meta, loses a torn last line, and tells which seeds are done."""
    path = pathlib.Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    done = set()
    if path.exists() and path.stat().st_size and resume_ok:
        old, runs = load_results(path)
        if old != meta:
            raise SystemExit(f"{path} has a different meta; remove it or choose another --out")
        data = path.read_bytes()
        keep = data[: data.rfind(b"\n") + 1]
        if len(keep) != len(data):
            path.write_bytes(keep)
        done = set(runs)
    else:
        path.write_text(dumps({"meta": meta}) + "\n")
    return done


def append_runs(path, runs):
    with open(path, "a") as f:
        for r in sorted(runs, key=lambda r: r["seed"]):
            f.write(dumps(r) + "\n")
        f.flush()
        os.fsync(f.fileno())


def registry_read(path):
    path = pathlib.Path(path)
    if not path.exists():
        return []
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def cmd_run(args):
    import torch

    first, count = panel_seeds(args.panel, args.n)
    ckpt = pathlib.Path(args.ckpt).expanduser()
    sha = file_sha256(ckpt)
    meta = make_meta("net", args.panel, first, count, ckpt_sha256=sha, weights=args.weights,
                     batch=args.batch, record=2)
    default = ROOT / "target/e2e-eval" / f"{sha[:12]}_{args.weights}_{args.panel}.jsonl"
    out = pathlib.Path(args.out) if args.out else default
    test = args.panel == "TEST"
    if test:
        registry = pathlib.Path(args.registry).expanduser()
        key = {"ckpt_sha256": sha, "weights": args.weights}
        started = any(all(r.get(k) == v for k, v in key.items()) for r in registry_read(registry))
        mine = False
        if out.exists() and out.stat().st_size:
            mine = load_results(out)[0] == meta
        if started and not mine and not args.i_know:
            raise SystemExit("TEST was already read for this checkpoint and weights "
                             "(--i-know to run it again)")
        if not started:
            registry.parent.mkdir(parents=True, exist_ok=True)
            with open(registry, "a") as f:
                f.write(dumps({**key, "out": str(out), "time": int(time.time())}) + "\n")
    device = torch.device(args.device)
    done = start_file(out, meta, resume_ok=True)
    seeds = [s for s in range(first, first + count)]
    chunks = [seeds[i:i + args.batch] for i in range(0, count, args.batch)]
    todo = [c for c in chunks if not all(s in done for s in c)]
    if todo:
        net = load_policy(ckpt, args.weights, device)
        t0 = time.time()
        for k, chunk in enumerate(todo):
            if args.stop_after is not None and k >= args.stop_after:
                raise SystemExit("stopped (--stop-after)")
            append_runs(out, play_batch(net, chunk, device, args.threads))
            if not test:
                print(f"batch {k + 1}/{len(todo)}  {time.time() - t0:.0f}s", flush=True)
    meta_, runs = load_results(out)
    if len(runs) != count:
        raise SystemExit(f"{out}: {len(runs)} runs, expected {count}")
    verified = None if args.no_replay else replay_file(out)
    if test:
        report_sealed(out, runs, verified)
    else:
        print_summary(meta_, runs)
        if verified is not None:
            print_replay(verified)
    return 0


def report_sealed(path, runs, verified):
    ends = {}
    for r in runs.values():
        ends[r["end"]] = ends.get(r["end"], 0) + 1
    wins = sum(r["win"] for r in runs.values())
    print(f"TEST runs {len(runs)} wins {wins} ends {dumps(ends)}")
    if verified is not None:
        print_replay(verified)
    print(f"sha256 {file_sha256(path)}")


# ---- the search-bot arm ----

BOT_END = {"victory": "won", "death": "died", "cap": "capped", "stuck": "stuck",
           "stopped": "unsupported"}


def parse_bot_rows(path):
    runs = []
    for line in pathlib.Path(path).read_text().splitlines():
        f = line.split("\t")
        if len(f) < 4 or not f[0].startswith("RUN"):
            continue
        end = BOT_END.get(f[1], f[1])
        act = int(f[2].split()[1])
        floor = int(f[3].split()[1])
        runs.append({
            "seed": int(f[0][3:]), "win": int(end == "won"), "floor": floor, "end": end,
            "bosses": max(0, act - 1) + (1 if end == "won" else 0),
            "boss_acts": 0, "kf": [], "nan": 0,
            "hash": hashlib.sha256(line.encode()).hexdigest()[:16],
        })
    return sorted(runs, key=lambda r: r["seed"])


def cmd_bot(args):
    """Runs `sts2-diff bot` (Necrobinder A10) over a panel; the same result format. `bosses`
    counts the acts the bot cleared (its rows give the act reached, not each kill), `kf` is empty
    and wins carry no action list: the bot replays its own wins (`replayed` in its summary)."""
    first, count = panel_seeds(args.panel, args.n)
    meta = make_meta("bot", args.panel, first, count, bot=str(args.bin.name),
                     bot_args="bot --character Necrobinder --ascension 10")
    out = pathlib.Path(args.out) if args.out else ROOT / f"target/e2e-eval/bot_{args.panel}.jsonl"
    if args.panel == "TEST":
        registry = pathlib.Path(args.registry).expanduser()
        key = {"ckpt_sha256": "search-bot", "weights": "-"}
        if any(r.get("ckpt_sha256") == "search-bot" for r in registry_read(registry)) \
                and not args.i_know:
            raise SystemExit("TEST was already read for the search bot (--i-know to run again)")
        registry.parent.mkdir(parents=True, exist_ok=True)
        with open(registry, "a") as f:
            f.write(dumps({**key, "out": str(out), "time": int(time.time())}) + "\n")
    work = ROOT / "target/e2e-eval" / f"botwork_{args.panel}_{first}_{count}"
    work.mkdir(parents=True, exist_ok=True)
    cmd = [str(args.bin), "bot", "--first", str(first), "--count", str(count), "--character",
           CHARACTER, "--ascension", str(ASCENSION), "--jobs", str(args.jobs), "--out", str(work)]
    print(" ".join(cmd), flush=True)
    res = subprocess.run(cmd, capture_output=True, text=True)
    if res.returncode != 0:
        raise SystemExit(f"sts2-diff bot failed:\n{res.stderr[-2000:]}")
    runs = parse_bot_rows(work / "rows.tsv")
    if len(runs) != count:
        raise SystemExit(f"the bot reported {len(runs)} runs, expected {count} "
                         f"(see {work}/summary.txt)")
    summary = (work / "summary.txt").read_text()
    meta["bot_summary"] = summary.splitlines()[0]
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(dumps({"meta": meta}) + "\n" + "".join(dumps(r) + "\n" for r in runs))
    if args.panel == "TEST":
        report_sealed(out, {r["seed"]: r for r in runs}, None)
    else:
        print_summary(meta, {r["seed"]: r for r in runs})
        print(meta["bot_summary"])
    return 0


# ---- replay ----


def replay_runs(runs, env_opts, group=256):
    """Re-runs each run's action list from its seed. Returns `{seed: (ended, won)}`: the run
    ended exactly on its last action, and that end was a victory."""
    from sts2 import HOLD, Sts2RunEnvs
    from sts2.ffi import Sts2Error

    runs = sorted(runs, key=lambda r: r["seed"])
    opts = {k: v for k, v in env_opts.items() if k not in ("priv", "character", "ascension",
                                                            "combat")}
    result = {}

    def play(part):
        acts = [unpack(r["actions"]) for r in part]
        n = len(part)
        env = Sts2RunEnvs(n, seeds=np.asarray([r["seed"] for r in part], dtype=np.uint64),
                          character=env_opts["character"], ascension=env_opts["ascension"],
                          combat=env_opts["combat"], threads=4, **opts)
        try:
            ended = np.zeros(n, dtype=bool)
            exact = np.zeros(n, dtype=bool)
            won = np.zeros(n, dtype=bool)
            for t in range(max((len(a) for a in acts), default=0)):
                step = np.array([a[t] if t < len(a) and not ended[i] else HOLD
                                 for i, a in enumerate(acts)], dtype=np.uint16)
                env.step(step)
                for i in np.nonzero(env.done.astype(bool) & ~ended)[0]:
                    ended[i] = True
                    exact[i] = t == len(acts[i]) - 1
                    won[i] = bool(env.won[i])
        finally:
            env.close()
        return {r["seed"]: (bool(exact[i]), bool(won[i])) for i, r in enumerate(part)}

    for lo in range(0, len(runs), group):
        part = runs[lo:lo + group]
        try:
            result.update(play(part))
        except Sts2Error:  # an illegal action: find which runs it is
            for r in part:
                try:
                    result.update(play([r]))
                except Sts2Error:
                    result[r["seed"]] = (False, False)
    return result


def replay_file(path):
    """Replays each win's action list in the engine. Returns `(wins, replayed, bad)` (`bad`: the
    seeds that did not end in victory on their last action, or whose list does not match its
    hash), or None for the bot arm."""
    meta, runs = load_results(path)
    if meta.get("kind") == "bot":
        return None
    wins = [r for r in runs.values() if r["win"]]
    bad = [r["seed"] for r in wins
           if "actions" not in r or action_hash(unpack(r["actions"])) != r["hash"]]
    got = replay_runs([r for r in wins if r["seed"] not in bad], meta["env"])
    bad += [s for s, (exact, won) in got.items() if not (exact and won)]
    return len(wins), len(wins) - len(bad), sorted(bad)


def print_replay(result):
    if result is None:
        return
    wins, ok, bad = result
    print(f"replayed wins {ok}/{wins}" + ("" if ok == wins else f"  INVALID READ: {sorted(bad)}"))


def cmd_replay(args):
    meta, _ = load_results(args.file)
    sealed(meta, True)
    result = replay_file(args.file)
    if result is None:
        print("bot arm: wins are replayed by the bot itself (see its summary)")
        return 0
    print_replay(result)
    return 0 if result[0] == result[1] else 1


# ---- summaries and comparisons ----


def stats_of(runs):
    rs = list(runs.values())
    n = len(rs)
    wins = sum(r["win"] for r in rs)
    ends = {}
    for r in rs:
        ends[r["end"]] = ends.get(r["end"], 0) + 1
    a2 = sum(reached_act2_boss(r) for r in rs)
    return dict(n=n, wins=wins, wr=wins / n if n else 0.0, wilson=wilson(wins, n),
                floor=sum(r["floor"] for r in rs) / n if n else 0.0, ends=ends,
                a2=a2, a2_rate=a2 / n if n else 0.0,
                a9=sum(map(a9_est, rs)), a9_rate=sum(map(a9_est, rs)) / n if n else 0.0, nan=sum(r["nan"] for r in rs),
                caps=ends.get("capped", 0) / n if n else 0.0,
                bosses=sum(r["bosses"] for r in rs))


def killers(runs, top=15):
    """The encounters that ended the most runs: `(encounter, deaths, reached, act)` rows, most
    deaths first. Reach counts the runs that entered the encounter at least once."""
    deaths, reached, acts = {}, {}, {}
    for r in runs.values():
        if r.get("death"):
            deaths[r["death"]] = deaths.get(r["death"], 0) + 1
        for e, a in zip(r.get("fights", ()), r.get("fight_acts", ())):
            acts[e] = a
        for e in set(r.get("fights", ())):
            reached[e] = reached.get(e, 0) + 1
    order = sorted(deaths, key=lambda e: (-deaths[e], e))[:top]
    return [(e, deaths[e], reached.get(e, 0), acts.get(e, "?")) for e in order]


def print_killers(runs):
    if not any("fights" in r for r in runs.values()):
        return
    rows = killers(runs)
    total = sum(1 for r in runs.values() if r.get("death"))
    print(f"top {len(rows)} killers of {total} combat deaths (* act boss; reach: runs that "
          f"entered it)")
    print(f"{'encounter':<44}{'deaths':>7}{'reached':>9}{'death/reach':>13}{'act':>5}")
    for e, d, n, a in rows:
        name = e + (" *" if e.endswith("_BOSS") else "")
        print(f"{name:<44}{d:>7}{n:>9}{100 * d / max(n, 1):>12.1f}%{a:>5}")


def print_summary(meta, runs):
    s = stats_of(runs)
    lo, hi = s["wilson"]
    print(f"{meta['kind']} {meta.get('weights', '')} {meta['panel']} runs {s['n']}  "
          f"wins {s['wins']} ({100 * s['wr']:.2f}%, Wilson {100 * lo:.2f}-{100 * hi:.2f})  "
          f"mean floor {s['floor']:.2f}  act2-boss reach {100 * s['a2_rate']:.1f}%  "
          f"A9-est {s['a9']} ({100 * s['a9_rate']:.2f}%)  bosses {s['bosses']}  caps {100 * s['caps']:.2f}%  nan {s['nan']}")
    print("ends " + dumps(s["ends"]))
    print_killers(runs)


def cmd_summary(args):
    meta, runs = load_results(args.file)
    sealed(meta, args.unseal)
    print_summary(meta, runs)
    return 0


def paired(a_runs, b_runs):
    seeds = sorted(set(a_runs) & set(b_runs))
    a = [a_runs[s] for s in seeds]
    b = [b_runs[s] for s in seeds]
    both = sum(x["win"] and y["win"] for x, y in zip(a, b))
    only_a = sum(x["win"] and not y["win"] for x, y in zip(a, b))
    only_b = sum(y["win"] and not x["win"] for x, y in zip(a, b))
    neither = len(seeds) - both - only_a - only_b
    out = dict(n=len(seeds), both=both, only_a=only_a, only_b=only_b, neither=neither)
    out["wr_diff"], out["wr_lo"], out["wr_hi"] = newcombe_paired(both, only_a, only_b, neither)
    out["p"] = mcnemar_exact(only_a, only_b)
    out["floor_diff"], out["floor_lo"], out["floor_hi"] = paired_mean(
        [x["floor"] for x in a], [y["floor"] for y in b])
    out["a9_a"] = sum(map(a9_est, a)) / len(a) if a else 0.0
    out["a9_b"] = sum(map(a9_est, b)) / len(b) if b else 0.0
    out["a2_a"] = sum(map(reached_act2_boss, a)) / len(a) if a else 0.0
    out["a2_b"] = sum(map(reached_act2_boss, b)) / len(b) if b else 0.0
    out["wr_a"] = (both + only_a) / len(a) if a else 0.0
    out["wr_b"] = (both + only_b) / len(b) if b else 0.0
    return out


def print_paired(name_a, name_b, p):
    print(f"paired {name_a} - {name_b} on {p['n']} shared seeds")
    print(f"  win rate {100 * p['wr_a']:.2f}% vs {100 * p['wr_b']:.2f}%: diff "
          f"{100 * p['wr_diff']:+.2f}pp (Newcombe {100 * p['wr_lo']:+.2f} to "
          f"{100 * p['wr_hi']:+.2f})  McNemar exact p {p['p']:.4g}")
    print(f"  discordant: only {name_a} {p['only_a']}, only {name_b} {p['only_b']}  "
          f"(both {p['both']}, neither {p['neither']})")
    print(f"  mean floor diff {p['floor_diff']:+.3f} (95% CI {p['floor_lo']:+.3f} to "
          f"{p['floor_hi']:+.3f})")
    print(f"  act-2 boss reach {100 * p['a2_a']:.1f}% vs {100 * p['a2_b']:.1f}%")
    print(f"  A9-est {100 * p['a9_a']:.1f}% vs {100 * p['a9_b']:.1f}% (descriptive)")


def label(meta, path):
    return f"{meta['kind']}:{meta.get('weights', '-')}:{pathlib.Path(path).name}"


def cmd_compare(args):
    ma, ra = load_results(args.a)
    mb, rb = load_results(args.b)
    sealed(ma, args.unseal)
    sealed(mb, args.unseal)
    if ma["panel"] != mb["panel"] and not {ma["panel"], mb["panel"]} <= {"DEV-1000", "DEV-2000"}:
        raise SystemExit(f"never compare across panels: {ma['panel']} vs {mb['panel']}")
    print_paired("A", "B", paired(ra, rb))
    return 0


# ---- gates ----


def gate_lines(ok, lines):
    for line in lines:
        print("  " + line)
    print("PASS" if ok else "FAIL")
    return 0 if ok else 1


def need_panel(meta, names, gate):
    if meta["panel"] not in names:
        raise SystemExit(f"{gate} reads {' or '.join(names)}, not {meta['panel']}")


def first_n(runs, n):
    """The runs of the panel's first `n` seeds (DEV-1000 inside a DEV-2000 file)."""
    return {s: r for s, r in runs.items() if s < PANELS["DEV-1000"][0] + n}


def cmd_gate(args):
    g, files = args.gate, args.files
    loaded = [load_results(f) for f in files]
    for m, _ in loaded:
        sealed(m, args.unseal)
    want = {"K1": 1, "K2": 2, "K3": 2, "K4": 2}[g]
    if len(files) != want:
        raise SystemExit(f"{g} takes {want} file(s)" + (": OLD NEW" if want == 2 else ""))
    print(f"gate {g}")
    for (m, runs), f in zip(loaded, files):
        st = stats_of(runs)
        print(f"  A9-est {100 * st['a9_rate']:.2f}% on {st['n']} runs of {label(m, f)} "
              f"(descriptive, not a gate)")
    if g == "K1":
        (m, runs), = loaded
        need_panel(m, ("DEV-1000",), g)
        s = stats_of(runs)
        ok = s["floor"] >= 14 and s["nan"] == 0 and s["caps"] < 0.001
        return gate_lines(ok, [f"mean floor {s['floor']:.2f} (>= 14)", f"nan {s['nan']} (== 0)",
                               f"caps {100 * s['caps']:.2f}% (< 0.1%)"])
    (m_old, old), (m_new, new) = loaded
    if g == "K2":
        need_panel(m_old, ("DEV-1000",), g)
        need_panel(m_new, ("DEV-1000",), g)
        p = paired(new, old)
        ok = p["a2_a"] >= 0.40 and p["floor_diff"] >= 3
        return gate_lines(ok, [
            f"act-2 boss reach {100 * p['a2_a']:.1f}% (>= 40%)",
            f"mean floor {p['floor_diff']:+.2f} over K1 on {p['n']} paired seeds (>= +3; "
            f"CI {p['floor_lo']:+.2f} to {p['floor_hi']:+.2f})"])
    if g == "K3":
        # NEW: the 1B DEV-2000 read; OLD: the 500M DEV-1000 read, compared on shared seeds.
        need_panel(m_new, ("DEV-2000",), g)
        need_panel(m_old, ("DEV-1000", "DEV-2000"), g)
        s = stats_of(new)
        p = paired(new, old)
        ok = s["wr"] >= 0.03 and p["only_a"] > p["only_b"] and p["p"] < 0.05
        return gate_lines(ok, [
            f"DEV-2000 win rate {100 * s['wr']:.2f}% (>= 3%)",
            f"vs the 500M read on {p['n']} shared seeds: {100 * p['wr_diff']:+.2f}pp, "
            f"discordant {p['only_a']}:{p['only_b']}, McNemar p {p['p']:.4g} (< 0.05, positive)"])
    need_panel(m_old, ("DEV-2000",), g)
    need_panel(m_new, ("DEV-2000",), g)
    p = paired(new, old)
    ok = p["wr_diff"] >= 0.02 and p["only_a"] > p["only_b"] and p["p"] < 0.05
    return gate_lines(ok, [
        f"gain {100 * p['wr_diff']:+.2f}pp (>= +2pp; Newcombe {100 * p['wr_lo']:+.2f} to "
        f"{100 * p['wr_hi']:+.2f}), discordant {p['only_a']}:{p['only_b']}, "
        f"McNemar p {p['p']:.4g} (< 0.05)"])


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    panels = ["SMOKE", *PANELS]

    r = sub.add_parser("run", help="greedy panel runner")
    r.add_argument("ckpt")
    r.add_argument("--weights", default="raw", choices=["raw", "ema0.996", "ema0.999"])
    r.add_argument("--panel", required=True, choices=panels)
    r.add_argument("--n", type=int, help="SMOKE: the number of seeds (default 1000)")
    r.add_argument("--out")
    r.add_argument("--batch", type=int, default=512, help="runs per batch (part of the meta)")
    r.add_argument("--threads", type=int, default=14)
    r.add_argument("--device", default="cuda" if _cuda() else "cpu")
    r.add_argument("--registry", default=str(DEFAULT_REGISTRY))
    r.add_argument("--i-know", action="store_true", help="read TEST again")
    r.add_argument("--no-replay", action="store_true")
    r.add_argument("--stop-after", type=int, help="stop after this many batches (for tests)")
    r.set_defaults(fn=cmd_run)

    b = sub.add_parser("bot", help="the search-bot arm (CPU)")
    b.add_argument("--panel", required=True, choices=panels)
    b.add_argument("--n", type=int)
    b.add_argument("--out")
    b.add_argument("--jobs", type=int, default=8)
    b.add_argument("--bin", type=pathlib.Path, default=BOT_BIN)
    b.add_argument("--registry", default=str(DEFAULT_REGISTRY))
    b.add_argument("--i-know", action="store_true")
    b.set_defaults(fn=cmd_bot)

    s = sub.add_parser("summary")
    s.add_argument("file")
    s.add_argument("--unseal", action="store_true")
    s.set_defaults(fn=cmd_summary)

    c = sub.add_parser("compare", help="paired, A minus B")
    c.add_argument("a")
    c.add_argument("b")
    c.add_argument("--unseal", action="store_true")
    c.set_defaults(fn=cmd_compare)

    p = sub.add_parser("replay")
    p.add_argument("file")
    p.set_defaults(fn=cmd_replay)

    g = sub.add_parser("gate", help="K1 FILE | K2/K3/K4 OLD NEW")
    g.add_argument("gate", choices=["K1", "K2", "K3", "K4"])
    g.add_argument("files", nargs="+")
    g.add_argument("--unseal", action="store_true")
    g.set_defaults(fn=cmd_gate)

    args = ap.parse_args(argv)
    return args.fn(args)


def _cuda():
    try:
        import torch

        return torch.cuda.is_available()
    except ImportError:
        return False


if __name__ == "__main__":
    sys.exit(main())
