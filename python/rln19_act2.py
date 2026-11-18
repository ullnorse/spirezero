"""RLN19: why act-2 bosses kill NECRO1 (`docs/research/rln19-act2-bosses.md`).

    python python/rln19_act2.py harvest CKPT --panel DEV-1000 --out F   # greedy runs + boss logs
    python python/rln19_act2.py build F... --per 200 --out BENCH        # the fixed boss benchmark
    python python/rln19_act2.py rehash OLD_BENCH --out NEW_BENCH     # add sha_v1 to an older bench
    python python/rln19_act2.py bench CKPT --bench BENCH [--samples K] --out F
    python python/rln19_act2.py analyze --harvest F [--bench-out F] [--samples-out F]

**Harvest** plays greedy runs like `e2e_eval.py run` and, for every act-2 boss fight, writes one
record: the run seed, the action prefix that leads to the fight's first agent decision (base64
uint16), the sha256 of that obs row, the entry state (run header, deck, relics, belt) and one row
per agent decision in the fight (turn, HP, block, energy, visible incoming damage, Osty, enemy HP,
belt, the action's entity kind), the potions and enemy HP at the end, and whether the boss died.

**Benchmark** (`build`): the first `--per` harvested fights per boss in seed order. `bench`
replays each prefix in a fresh env (no net; HOLD keeps envs in step), checks the obs sha, then
plays the fight with the net (greedy, or `--samples K` sampled plays per fight) until the fight
ends. Score: the share of fights in which the boss died. Deterministic for greedy play; the obs
digest check refuses a prefix the engine no longer reproduces (an engine change: rebuild the bench).

The digest (`sha_v1`) is the sha256 of the observation's v1 sections only (`V1_BYTES`: the run header
and the combat state, i.e. the game state at the decision). The obs v2 section after them is encoder
input, not game state, and RLN21/22 grew it (ABI 9, 10) without any game change; a digest over the
whole row (the old `sha`) broke on every such change. `rehash` adds `sha_v1` to a bench built before
(replaying every prefix, and checking each fight's boss, floor and entry state against the file):

    python python/rln19_act2.py rehash OLD_BENCH --out NEW_BENCH

`--gpu-window` pauses the NECRO1 lane (`tools/e2e_lane.sh pause`), waits until `nvidia-smi` shows
< 3 GB used, runs only the net part on the GPU, and always resumes the lane. Envs use <= 4
threads.
"""

import argparse
import contextlib
import gzip
import hashlib
import json
import math
import os
import pathlib
import subprocess
import sys
import time

import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import e2e_eval as EV  # noqa: E402

ROOT = EV.ROOT
LANE = ROOT / "tools/e2e_lane.sh"
BOSSES = ("KAISER_CRAB_BOSS", "THE_INSATIABLE_BOSS", "KNOWLEDGE_DEMON_BOSS")
THREADS = int(os.environ.get("STS2_THREADS", "4"))

# Run header (`run_obs.rs`).
H_PHASE, H_HP, H_MAXHP, H_GOLD, H_DECK, H_RELICS = 3, 8, 10, 12, 16, 18
DECK_AT, DECK_SLOTS = 32, 80
BELT_AT, BELT_SLOTS = 480, 6
COMBAT_AT = 1750
# Combat section (`obs.rs`), relative to COMBAT_AT.
C_HP, C_BLOCK, C_ENERGY, C_TURN = 0, 4, 6, 12
C_CREATURES, CREATURE_BYTES, CREATURES = 24, 60, 8
C_INTENTS, INTENT_BYTES, INTENTS = 908, 5, 8
PHASE_COMBAT = 3


def _vocab(name):
    return (ROOT / f"crates/sts2-rl/vocab/{name}.txt").read_text().splitlines()


_CARD_TYPES = None


def card_types():
    """Card vocab id -> type (`target/rln19/card_types.tsv`, from
    `cargo run -p sts2-rl --example card_types`)."""
    global _CARD_TYPES
    if _CARD_TYPES is None:
        path = ROOT / "target/rln19/card_types.tsv"
        if not path.exists():
            out = subprocess.run(["cargo", "run", "-q", "--release", "-p", "sts2-rl", "--example",
                                  "card_types"], cwd=ROOT, capture_output=True, text=True,
                                 check=True).stdout
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(out)
        _CARD_TYPES = [line.split("\t")[1] for line in path.read_text().splitlines()]
    return _CARD_TYPES


def i16(row, at):
    return int(np.frombuffer(row[at:at + 2].tobytes(), dtype="<i2")[0])


def u16(row, at):
    return int(row[at]) | int(row[at + 1]) << 8


def entry_state(row):
    """The run header's state at a fight's start (HP and belt in the header are as of the start)."""
    names = _vocab("cards")
    types = card_types()
    deck = []
    for k in range(DECK_SLOTS):
        cid = u16(row, DECK_AT + 4 * k)
        if cid:
            deck.append((names[cid], types[cid], int(row[DECK_AT + 4 * k + 2])))
    count = lambda t: sum(1 for _, ty, _ in deck if ty == t)  # noqa: E731
    potions = sum(1 for k in range(BELT_SLOTS) if u16(row, BELT_AT + 2 * k))
    return {
        "hp": i16(row, H_HP), "max_hp": i16(row, H_MAXHP), "gold": i16(row, H_GOLD),
        "deck": u16(row, H_DECK), "relics": int(row[H_RELICS]), "potions": potions,
        "attacks": count("Attack"), "skills": count("Skill"), "powers": count("Power"),
        "curses": count("Curse"), "statuses": count("Status"),
        "upgraded": sum(1 for _, _, u in deck if u), "cards": [n + "+" * u for n, _, u in deck],
    }


_OSTY = None


def combat_row(row):
    """One decision's fight state: turn, HP, block, energy, visible incoming damage (attack
    intents, damage per hit x hits, after modifiers), Osty alive and HP, enemies' HP and max HP
    (alive ones), the belt's potions."""
    global _OSTY
    if _OSTY is None:
        _OSTY = _vocab("monsters").index("OSTY")
    c = COMBAT_AT
    incoming = 0
    for k in range(INTENTS):
        at = c + C_INTENTS + k * INTENT_BYTES
        if row[at]:
            incoming += max(0, i16(row, at + 1)) * int(row[at + 3])
    osty_alive, osty_hp, ehp, emax = 0, 0, 0, 0
    for k in range(CREATURES):
        at = c + C_CREATURES + k * CREATURE_BYTES
        mon = u16(row, at + 4)
        if not mon and not row[at]:
            continue
        side, alive = int(row[at + 1]), int(row[at])
        if mon == _OSTY:
            osty_alive, osty_hp = alive, i16(row, at + 6)
        elif side == 1 and alive:
            ehp += i16(row, at + 6)
            emax += i16(row, at + 8)
    belt = sum(1 for k in range(BELT_SLOTS) if u16(row, BELT_AT + 2 * k))
    return [u16(row, c + C_TURN), i16(row, c + C_HP), i16(row, c + C_BLOCK),
            i16(row, c + C_ENERGY), incoming, osty_alive, osty_hp, ehp, emax, belt]


ROW_FIELDS = ("turn", "hp", "block", "energy", "incoming", "osty", "osty_hp", "ehp", "emax",
              "belt", "kind")


V1_BYTES = 2996  # the obs v1 sections (`encode.V2_AT`); everything after is the v2 section


def sha(row):
    """Digest of the game state at a decision: the v1 obs bytes (not the v2 section, which is encoder
    input that obs-layout changes move without any game change)."""
    return hashlib.sha256(np.ascontiguousarray(row[:V1_BYTES]).tobytes()).hexdigest()[:24]


# ---- the GPU window ----


def gpu_used_mb():
    out = subprocess.run(["nvidia-smi", "--query-gpu=memory.used", "--format=csv,noheader,nounits"],
                         capture_output=True, text=True, check=True).stdout
    return int(out.split()[0])


@contextlib.contextmanager
def gpu_window(enabled, limit_s=540):
    """Pauses the lane until the block ends (always resumes); refuses to start the block if the
    GPU does not free up within 120 s."""
    if not enabled:
        yield
        return
    t0 = time.time()
    try:
        subprocess.run([str(LANE), "pause"], check=True, capture_output=True)
        while gpu_used_mb() >= 3000:
            if time.time() - t0 > 120:
                raise SystemExit("the GPU did not free up within 120 s")
            time.sleep(2)
        print(f"gpu window open ({gpu_used_mb()} MB used, waited {time.time() - t0:.0f}s)",
              flush=True)
        yield
    finally:
        subprocess.run([str(LANE), "resume"], check=False, capture_output=True)
        print(f"gpu window closed after {time.time() - t0:.0f}s (limit {limit_s}s)", flush=True)


def policy_actions(net, env, idx, device, sample=False, gen=None, temp=1.0):
    import torch

    from sts2.e2e import encode as E

    v2 = bool(getattr(net, "obs_v2", False))
    obs = torch.from_numpy(env.obs[idx]).to(device)
    masks = torch.from_numpy(env.masks.view(np.int64)[idx]).to(device)
    with torch.no_grad():
        tok = E.encode(obs, masks, v2=v2)
        with torch.autocast(device.type, dtype=torch.bfloat16, enabled=device.type == "cuda"):
            logits, _, _ = net.policy(tok)
        logits = logits.float().nan_to_num(nan=float("-inf"))
        legal = E.mask_bits(masks) if hasattr(E, "mask_bits") else None
        if legal is not None:
            logits = logits.masked_fill(~legal, float("-inf"))
        if sample:
            p = torch.softmax(logits / temp, -1)
            action = torch.multinomial(p, 1, generator=gen).squeeze(1)
        else:
            action = logits.argmax(-1)
    return action.cpu().numpy()


# ---- harvest ----


def harvest_batch(net, seeds, device):
    from sts2 import E2E, HOLD, Sts2RunEnvs
    from sts2.e2e import encode as E

    n = len(seeds)
    v2 = bool(getattr(net, "obs_v2", False))
    env = Sts2RunEnvs(n, seeds=np.asarray(seeds, dtype=np.uint64), character=EV.CHARACTER,
                      ascension=EV.ASCENSION, combat="agent", threads=THREADS,
                      **(dict(obs_v2=True) if v2 else {}), **E2E)
    E.check_layout(env.layout, v2=v2)
    kinds = EV_kinds()
    names = EV.encounter_names()
    cap = E2E["decision_cap"] + 8
    log = np.zeros((cap, n), dtype=np.uint16)
    steps = np.zeros(n, dtype=np.int64)
    finished = np.zeros(n, dtype=bool)
    cur = [None] * n  # the open boss fight's record
    out = []
    try:
        while not finished.all():
            active = np.nonzero(~finished)[0]
            for i in active:
                row = env.obs[i]
                f = EV.fight_of(row)
                in_boss = (f is not None and f[2] == 1 and names[f[1]] in BOSSES
                           and int(row[H_PHASE]) == PHASE_COMBAT)
                if in_boss and cur[i] is None:
                    cur[i] = {"seed": int(seeds[i]), "boss": names[f[1]], "floor": f[0],
                              "prefix": EV.pack(log[: steps[i], i]), "sha_v1": sha(row),
                              "entry": entry_state(row), "rows": [], "won": 0, "end": None}
            action = policy_actions(net, env, active, device)
            for j, i in enumerate(active):
                rec = cur[i]
                if rec is not None and rec["end"] is None:
                    r = combat_row(env.obs[i])
                    rec["rows"].append(r + [kinds[action[j]]])
            log[steps[active], active] = action
            steps[active] += 1
            step_actions = np.full(n, HOLD, dtype=np.uint16)
            step_actions[active] = action
            env.step(step_actions)
            ev = env.events
            for i in active:
                rec = cur[i]
                if rec is None or rec["end"] is not None:
                    continue
                done = bool(env.done[i])
                row = env.final_obs[i] if done else env.obs[i]
                if ev[i, 1] > 0:
                    rec["won"], rec["end"] = 1, "won"
                elif done:
                    rec["end"] = EV.RUN_ENDS_NAME(env.end[i])
                    rec["final"] = combat_row(row)
                elif int(row[H_PHASE]) != PHASE_COMBAT:
                    rec["end"] = "left"
                if rec["end"] is not None:
                    out.append(rec)
            for i in active[env.done[active].astype(bool)]:
                finished[i] = True
    finally:
        env.close()
    return out


def EV_kinds():
    from sts2 import action_table
    from sts2.env import ENTITY_KINDS

    t = action_table()
    return [ENTITY_KINDS[k] for k in t[:, 0]]


def _run_ends_name(code):
    from sts2.env import RUN_ENDS

    return RUN_ENDS[code]


EV.RUN_ENDS_NAME = _run_ends_name


def write_jsonl_gz(path, rows, meta):
    path = pathlib.Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    opener = gzip.open if path.suffix == ".gz" else open
    with opener(path, "wt") as f:
        f.write(EV.dumps({"meta": meta}) + "\n")
        for r in rows:
            f.write(EV.dumps(r) + "\n")


def read_jsonl(path):
    path = pathlib.Path(path)
    opener = gzip.open if path.suffix == ".gz" else open
    meta, rows = None, []
    with opener(path, "rt") as f:
        for line in f:
            obj = json.loads(line)
            if "meta" in obj:
                meta = obj["meta"]
            else:
                rows.append(obj)
    return meta, rows


def load_net(ckpt, weights):
    import torch

    torch.set_num_threads(THREADS)
    return EV.load_policy(ckpt, weights, torch.device("cpu"))


def cmd_harvest(args):
    import torch

    first, count = EV.panel_seeds(args.panel, args.n)
    seeds = list(range(first, first + count))
    net = load_net(args.ckpt, args.weights)
    rows = []
    with gpu_window(args.gpu_window):
        device = torch.device(args.device)
        net = net.to(device)
        t0 = time.time()
        for k in range(0, count, args.batch):
            rows += harvest_batch(net, seeds[k:k + args.batch], device)
            print(f"{k + args.batch}/{count} runs  {len(rows)} boss fights  "
                  f"{time.time() - t0:.0f}s", flush=True)
    meta = {"kind": "rln19-harvest", "panel": args.panel, "first": first, "count": count,
            "ckpt_sha256": EV.file_sha256(args.ckpt), "weights": args.weights,
            "commit": EV.code_commit()}
    write_jsonl_gz(args.out, rows, meta)
    by = {b: [r for r in rows if r["boss"] == b] for b in BOSSES}
    for b, rs in by.items():
        print(f"{b:24s} reached {len(rs):4d}  won {sum(r['won'] for r in rs):4d}")
    return 0


# ---- the benchmark ----


def cmd_build(args):
    fights = []
    metas = []
    for path in args.harvest:
        meta, rows = read_jsonl(path)
        metas.append(meta)
        fights += rows
    fights.sort(key=lambda r: r["seed"])
    bench = []
    for b in BOSSES:
        pick = [r for r in fights if r["boss"] == b][: args.per]
        if len(pick) < args.per:
            raise SystemExit(f"{b}: only {len(pick)} fights harvested, need {args.per}")
        for k, r in enumerate(pick):
            bench.append({"id": f"{b}:{k:03d}", "seed": r["seed"], "boss": b, "floor": r["floor"],
                          "prefix": r["prefix"], "sha_v1": r["sha_v1"], "entry": r["entry"],
                          "source_won": r["won"]})
    meta = {"kind": "rln19-act2-boss-bench", "version": 2, "per": args.per, "bosses": BOSSES,
            "sources": metas, "commit": EV.code_commit()}
    write_jsonl_gz(args.out, bench, meta)
    print(f"{len(bench)} fights -> {args.out}")
    return 0


def replay_prefixes(entries, copies, check=True):
    """A run env per (entry, copy) replayed to its fight's first agent decision; checks `sha_v1`
    (`check=False`: no check, for `rehash`)."""
    from sts2 import E2E, HOLD, Sts2RunEnvs

    rows = [(e, c) for e in entries for c in range(copies)]
    n = len(rows)
    if check and any("sha_v1" not in e for e, _ in rows):
        raise SystemExit("this bench has no sha_v1 digests (built before RLN23): `rehash` it")
    seeds = np.asarray([e["seed"] for e, _ in rows], dtype=np.uint64)
    env = Sts2RunEnvs(n, seeds=seeds, character=EV.CHARACTER, ascension=EV.ASCENSION,
                      combat="agent", threads=THREADS, obs_v2=True, **E2E)
    if int(env.layout.v2_at) != V1_BYTES:
        raise SystemExit(f"the v2 section starts at {int(env.layout.v2_at)}, not {V1_BYTES}: "
                         "the obs v1 layout moved (a real engine change)")
    prefixes = [EV.unpack(e["prefix"]) for e, _ in rows]
    longest = max(len(p) for p in prefixes)
    acts = np.full((longest, n), HOLD, dtype=np.uint16)
    for i, p in enumerate(prefixes):
        acts[: len(p), i] = p
    for t in range(longest):
        env.step(acts[t])
        if env.done.any():
            raise SystemExit("a prefix ended its run: the engine no longer reproduces it")
    if check:
        bad = [rows[i][0]["id"] for i in range(n) if sha(env.obs[i]) != rows[i][0]["sha_v1"]]
        if bad:
            env.close()
            raise SystemExit(f"{len(bad)} prefixes reach a different state (e.g. {bad[:3]}): "
                             "the engine changed, rebuild the benchmark")
    return env, rows


def cmd_rehash(args):
    """Adds `sha_v1` to a bench built before RLN23, after checking that every prefix still reaches
    the same fight (boss, floor, entry state as stored); all other fields are copied unchanged."""
    meta, entries = read_jsonl(args.bench)
    names = EV.encounter_names()
    out, t0 = [], time.time()
    for k in range(0, len(entries), args.batch):
        chunk = entries[k:k + args.batch]
        env, rows = replay_prefixes(chunk, 1, check=False)
        for i, (e, _) in enumerate(rows):
            row = env.obs[i]
            f = EV.fight_of(row)
            if (f is None or names[f[1]] != e["boss"] or f[0] != e["floor"]
                    or int(row[H_PHASE]) != PHASE_COMBAT or entry_state(row) != e["entry"]):
                raise SystemExit(f"{e['id']}: the prefix reaches a different fight or entry state")
            out.append({**e, "sha_v1": sha(row)})
        env.close()
        print(f"rehashed {len(out)}/{len(entries)}  {time.time() - t0:.0f}s", flush=True)
    new_meta = {**meta, "version": 2, "rehash_of_sha256": EV.file_sha256(args.bench),
                "rehash_commit": EV.code_commit()}
    write_jsonl_gz(args.out, out, new_meta)
    print(f"{len(out)} fights -> {args.out}")
    return 0


def enemies(row):
    """(monster vocab id, HP, block) of each live enemy."""
    out = []
    for k in range(CREATURES):
        at = COMBAT_AT + C_CREATURES + k * CREATURE_BYTES
        if row[at] and row[at + 1] == 1:
            out.append((u16(row, at + 4), i16(row, at + 6), i16(row, at + 10)))
    return out


def play_fights(net, env, device, sample=False, seed=0, temp=1.0, log=False):
    """Plays each env's fight to its end; per env (won, turns, final combat row)."""
    import torch

    from sts2 import HOLD

    n = env.n
    gen = None
    if sample:
        gen = torch.Generator(device=device)
        gen.manual_seed(seed)
    open_ = np.ones(n, dtype=bool)
    won = np.zeros(n, dtype=np.int64)
    last = [None] * n
    logs = [[] for _ in range(n)] if log else None
    kinds = EV_kinds() if log else None
    while open_.any():
        idx = np.nonzero(open_)[0]
        for i in idx:
            last[i] = combat_row(env.obs[i])
        action = policy_actions(net, env, idx, device, sample=sample, gen=gen, temp=temp)
        if log:
            for j, i in enumerate(idx):
                logs[i].append(last[i] + [kinds[action[j]], int(action[j]), enemies(env.obs[i])])
        acts = np.full(n, HOLD, dtype=np.uint16)
        acts[idx] = action
        env.step(acts)
        ev = env.events
        for i in idx:
            done = bool(env.done[i])
            if ev[i, 1] > 0:
                won[i], open_[i] = 1, False
            elif done or int(env.obs[i][H_PHASE]) != PHASE_COMBAT:
                open_[i] = False
                if done:
                    last[i] = combat_row(env.final_obs[i])
    return (won, last, logs) if log else (won, last)


def cmd_bench(args):
    import torch

    meta, entries = read_jsonl(args.bench)
    net = load_net(args.ckpt, args.weights)
    copies = max(1, args.samples)
    results = []
    chunk = max(1, args.batch // copies)
    t0 = time.time()
    envs = []
    for k in range(0, len(entries), chunk):  # CPU only, outside the GPU window
        envs.append(replay_prefixes(entries[k:k + chunk], copies))
        print(f"replayed {min(k + chunk, len(entries))}/{len(entries)}  {time.time() - t0:.0f}s",
              flush=True)
    with gpu_window(args.gpu_window):
        device = torch.device(args.device)
        net = net.to(device)
        for b, (env, rows) in enumerate(envs):
            won, last, logs = play_fights(net, env, device, sample=args.samples > 0,
                                          seed=args.seed + b, temp=args.temp, log=True)
            env.close()
            for i, (e, c) in enumerate(rows):
                r = {"id": e["id"], "boss": e["boss"], "copy": c, "won": int(won[i]),
                     "final": last[i]}
                if args.log:
                    r["rows"] = logs[i]
                results.append(r)
    out_meta = {"kind": "rln19-bench-result", "bench_sha256": EV.file_sha256(args.bench),
                "ckpt_sha256": EV.file_sha256(args.ckpt), "weights": args.weights,
                "samples": args.samples, "temp": args.temp, "seed": args.seed}
    if args.out:
        write_jsonl_gz(args.out, results, out_meta)
    print_bench(results, args.samples)
    return 0


def print_bench(results, samples):
    print(f"{'boss':24s} {'fights':>6s} {'win%':>6s}  Wilson 95%" +
          ("   any-of-K win%" if samples > 1 else ""))
    tot_k = tot_n = 0
    for b in BOSSES + ("ALL",):
        rs = [r for r in results if b == "ALL" or r["boss"] == b]
        k, n = sum(r["won"] for r in rs), len(rs)
        if b != "ALL":
            tot_k, tot_n = tot_k + k, tot_n + n
        lo, hi = EV.wilson(k, n)
        line = f"{b:24s} {n:6d} {100 * k / max(n, 1):6.1f}  {100 * lo:4.1f}-{100 * hi:4.1f}"
        if samples > 1:
            ids = {}
            for r in rs:
                ids[r["id"]] = ids.get(r["id"], 0) | r["won"]
            line += f"   {100 * sum(ids.values()) / max(len(ids), 1):6.1f}"
        print(line)


# ---- analysis ----


def turns_of(rec):
    """Per turn: start HP, last-decision state, whether the agent chose end_turn, the damage
    taken over the enemy turn (HP at the next turn's first decision, or at the end)."""
    rows = [dict(zip(ROW_FIELDS, r)) for r in rec["rows"]]
    turns = {}
    for r in rows:
        t = turns.setdefault(r["turn"], {"first": r, "last": r, "end_turn": False, "potions": 0})
        t["last"] = r
        if r["kind"] == "end_turn":
            t["end_turn"] = True
            t["end"] = r
        if r["kind"] == "combat_potion":
            t["potions"] += 1
    order = sorted(turns)
    for a, b in zip(order, order[1:]):
        turns[a]["taken"] = turns[a]["last"]["hp"] - turns[b]["first"]["hp"]
    if order and rec["end"] == "died":  # `final_obs` is the last decision's row: HP is 0 now
        turns[order[-1]]["taken"] = turns[order[-1]]["last"]["hp"]
    return [turns[t] for t in order]


def fight_stats(rec):
    ts = turns_of(rec)
    ends = [t for t in ts if t["end_turn"]]
    full = [t for t in ts if "taken" in t]
    inc = sum(t["last"]["incoming"] for t in full)
    blk = sum(min(t["last"]["block"], t["last"]["incoming"]) for t in full)
    taken = sum(max(0, t["taken"]) for t in full)
    first = rec["rows"][0] if rec["rows"] else None
    entry = rec["entry"]
    final = rec.get("final")
    used = sum(1 for r in rec["rows"] if r[-1] == "combat_potion")
    return {
        "turns": max((t["last"]["turn"] for t in ts), default=0),
        "taken_per_turn": taken / max(len(full), 1),
        "incoming_per_turn": inc / max(len(full), 1),
        "taken_over_incoming": taken / inc if inc else float("nan"),
        "block_cover": blk / inc if inc else float("nan"),
        "unused_energy": (sum(t["end"]["energy"] for t in ends) / len(ends)) if ends else 0.0,
        "waste_turns": (sum(1 for t in ends if t["end"]["energy"] >= 1) / len(ends)) if ends else 0,
        "explicit_end": len(ends) / max(len(ts), 1),
        "potions_start": first[9] if first else entry["potions"],
        "potions_used": used,
        "potions_left": (final[9] if final else None),
        "osty_alive": (sum(t["first"]["osty"] for t in ts) / max(len(ts), 1)),
        "enemy_left": (final[7] / final[8] if final and final[8] else 0.0),
        "hp_entry": entry["hp"] / max(entry["max_hp"], 1),
    }


def auc(scores, labels):
    """Mann-Whitney AUC."""
    pos = [s for s, y in zip(scores, labels) if y]
    neg = [s for s, y in zip(scores, labels) if not y]
    if not pos or not neg:
        return float("nan")
    order = sorted((s, y) for s, y in zip(scores, labels))
    rank, ranks = 1, {}
    i = 0
    while i < len(order):
        j = i
        while j < len(order) and order[j][0] == order[i][0]:
            j += 1
        for k in range(i, j):
            ranks.setdefault(order[k][0], (rank + rank + j - i - 1) / 2)
        rank += j - i
        i = j
    rpos = sum(ranks[s] for s in pos)
    return (rpos - len(pos) * (len(pos) + 1) / 2) / (len(pos) * len(neg))


ENTRY_FEATURES = ("hp_frac", "hp", "max_hp", "deck", "attacks", "skills", "powers", "curses",
                  "upgraded", "relics", "potions", "gold")


def entry_vec(e):
    return [e["hp"] / max(e["max_hp"], 1), e["hp"], e["max_hp"], e["deck"], e["attacks"],
            e["skills"], e["powers"], e["curses"], e["upgraded"], e["relics"], e["potions"],
            e["gold"]]


def cv_logistic_auc(x, y, folds=5, l2=1.0, seed=0):
    """Out-of-fold AUC of an L2 logistic regression on standardized features (numpy, Newton)."""
    x = np.asarray(x, dtype=np.float64)
    y = np.asarray(y, dtype=np.float64)
    n = len(y)
    rng = np.random.default_rng(seed)
    fold = rng.permutation(n) % folds
    pred = np.zeros(n)
    for f in range(folds):
        tr, te = fold != f, fold == f
        mu, sd = x[tr].mean(0), x[tr].std(0) + 1e-9
        a = np.c_[np.ones(tr.sum()), (x[tr] - mu) / sd]
        w = np.zeros(a.shape[1])
        for _ in range(50):
            p = 1 / (1 + np.exp(-a @ w))
            g = a.T @ (p - y[tr]) + l2 * np.r_[0, w[1:]]
            h = a.T @ (a * (p * (1 - p))[:, None]) + l2 * np.diag(np.r_[0, np.ones(len(w) - 1)])
            w -= np.linalg.solve(h, g)
        b = np.c_[np.ones(te.sum()), (x[te] - mu) / sd]
        pred[te] = b @ w
    return auc(pred.tolist(), y.astype(int).tolist())


def cmd_analyze(args):
    fights = []
    for path in args.harvest:
        fights += read_jsonl(path)[1]
    ends = {}
    for f in fights:
        ends[f["end"]] = ends.get(f["end"], 0) + 1
    print(f"fight ends {ends}")
    fights = [f for f in fights if f["end"] in ("won", "died")]
    print(f"## entry state ({args.harvest})\n")
    keys = ("hp", "max_hp", "hp_pct", "deck", "attacks", "skills", "powers", "upgraded",
            "curses", "relics", "potions", "gold")
    print("| boss | outcome | n | " + " | ".join(keys) + " |")
    print("|---|---|---:|" + "---:|" * len(keys))
    for b in BOSSES:
        for won in (1, 0):
            rs = [f for f in fights if f["boss"] == b and f["won"] == won]
            if not rs:
                continue
            m = lambda k: np.mean([f["entry"][k] for f in rs])  # noqa: E731
            vals = [m("hp"), m("max_hp"),
                    100 * np.mean([f["entry"]["hp"] / f["entry"]["max_hp"] for f in rs]),
                    m("deck"), m("attacks"), m("skills"), m("powers"), m("upgraded"), m("curses"),
                    m("relics"), m("potions"), m("gold")]
            print(f"| {b} | {'won' if won else 'lost'} | {len(rs)} | "
                  + " | ".join(f"{v:.1f}" for v in vals) + " |")
    print("\n## entry-state predictability (out-of-fold AUC, 5 folds)\n")
    print("| boss | n | win% | AUC all entry features | AUC hp% only | AUC deck only |")
    print("|---|---:|---:|---:|---:|---:|")
    for b in BOSSES + ("ALL",):
        rs = [f for f in fights if b == "ALL" or f["boss"] == b]
        y = [f["won"] for f in rs]
        x = [entry_vec(f["entry"]) for f in rs]
        if b == "ALL":
            x = [v + [f["boss"] == bb for bb in BOSSES] for v, f in zip(x, rs)]
        hp = [f["entry"]["hp"] / f["entry"]["max_hp"] for f in rs]
        deck_only = [[v[3], v[4], v[5], v[6], v[7], v[8]] for v in x]
        print(f"| {b} | {len(rs)} | {100 * np.mean(y):.1f} | {cv_logistic_auc(x, y):.2f} | "
              f"{auc(hp, y):.2f} | {cv_logistic_auc(deck_only, y):.2f} |")
    print("\n## card presence: win rate with vs without (all three bosses, cards in >= 15% of decks)\n")
    allf = fights
    names = sorted({c.rstrip("+") for f in allf for c in f["entry"]["cards"]})
    rowsc = []
    for c in names:
        has = [f for f in allf if any(x.rstrip("+") == c for x in f["entry"]["cards"])]
        if len(has) < 0.15 * len(allf) or len(has) == len(allf):
            continue
        no = [f for f in allf if f not in has]
        rowsc.append((np.mean([f["won"] for f in has]) - np.mean([f["won"] for f in no]), c,
                      len(has), np.mean([f["won"] for f in has]), np.mean([f["won"] for f in no])))
    rowsc.sort()
    print("| card | decks | win% with | win% without | diff |")
    print("|---|---:|---:|---:|---:|")
    for d, c, n, a, bb in rowsc[:6] + rowsc[-6:]:
        print(f"| {c} | {n} | {100 * a:.0f} | {100 * bb:.0f} | {100 * d:+.0f} |")
    print("\n## fight play (means per fight)\n")
    skeys = ("turns", "incoming_per_turn", "taken_per_turn", "taken_over_incoming", "block_cover",
             "unused_energy", "waste_turns", "explicit_end", "potions_start", "potions_used",
             "potions_left", "osty_alive", "enemy_left")
    print("| boss | outcome | n | " + " | ".join(skeys) + " |")
    print("|---|---|---:|" + "---:|" * len(skeys))
    for b in BOSSES:
        for won in (1, 0):
            rs = [fight_stats(f) for f in fights if f["boss"] == b and f["won"] == won]
            if not rs:
                continue
            vals = []
            for k in skeys:
                v = [r[k] for r in rs if r[k] is not None and not (isinstance(r[k], float)
                                                                   and math.isnan(r[k]))]
                vals.append(f"{np.mean(v):.2f}" if v else "-")
            print(f"| {b} | {'won' if won else 'lost'} | {len(rs)} | " + " | ".join(vals) + " |")
    print("\n## lost fights: how they end\n")
    print("| boss | lost | died with potions | enemy HP left <= 25% | enemy HP left > 50% | "
          "Osty dead at last turn |")
    print("|---|---:|---:|---:|---:|---:|")
    for b in BOSSES:
        rs = [f for f in fights if f["boss"] == b and not f["won"]]
        st = [fight_stats(f) for f in rs]
        n = len(rs)
        pot = sum(1 for s in st if (s["potions_left"] or 0) > 0)
        close = sum(1 for s in st if s["enemy_left"] <= 0.25)
        far = sum(1 for s in st if s["enemy_left"] > 0.5)
        od = sum(1 for f in rs if f["rows"] and turns_of(f)[-1]["first"]["osty"] == 0)
        print(f"| {b} | {n} | {pot} ({100 * pot / max(n, 1):.0f}%) | {close} | {far} | "
              f"{od} ({100 * od / max(n, 1):.0f}%) |")
    print("\n## per turn, lost vs won (turn index 1..8: mean incoming / taken / block / unused "
          "energy at end_turn)\n")
    for b in BOSSES:
        print(f"\n{b}\n")
        print("| turn | won: incoming | taken | block | energy left | lost: incoming | taken | "
              "block | energy left | lost alive |")
        print("|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
        per = {}
        for won in (1, 0):
            for f in fights:
                if f["boss"] != b or f["won"] != won:
                    continue
                for k, t in enumerate(turns_of(f)[:8]):
                    d = per.setdefault((won, k), [[], [], [], [], 0])
                    d[0].append(t["last"]["incoming"])
                    if "taken" in t:
                        d[1].append(max(0, t["taken"]))
                    d[2].append(t["last"]["block"])
                    if t["end_turn"]:
                        d[3].append(t["end"]["energy"])
                    d[4] += 1
        mm = lambda v: f"{np.mean(v):.1f}" if v else "-"  # noqa: E731
        for k in range(8):
            w, lo = per.get((1, k)), per.get((0, k))
            cells = []
            for d in (w, lo):
                cells += [mm(d[0]), mm(d[1]), mm(d[2]), mm(d[3])] if d else ["-"] * 4
            cells.append(str(lo[4]) if lo else "0")
            print(f"| {k + 1} | " + " | ".join(cells) + " |")
    return 0


def cmd_resolver(args):
    """The same fights played by the engine's beam-search `Greedy` resolver (no net, CPU):
    `cargo run --release -p sts2-rl --example boss_resolver`."""
    _, entries = read_jsonl(args.bench)
    tsv = "".join(f"{e['id']}\t{e['seed']}\t{','.join(map(str, EV.unpack(e['prefix'])))}\n"
                  for e in entries)
    t0 = time.time()
    res = subprocess.run(["cargo", "run", "-q", "--release", "-p", "sts2-rl", "--example",
                          "boss_resolver"], cwd=ROOT, input=tsv, capture_output=True, text=True)
    if res.returncode:
        raise SystemExit(res.stderr[-3000:])
    boss = {e["id"]: e["boss"] for e in entries}
    results = []
    for line in res.stdout.splitlines():
        i, w, hp = line.split("\t")
        results.append({"id": i, "boss": boss[i], "copy": 0, "won": int(w), "hp": int(hp)})
    print(f"resolver played {len(results)} fights in {time.time() - t0:.0f}s")
    if args.out:
        write_jsonl_gz(args.out, results, {"kind": "rln19-resolver",
                                           "bench_sha256": EV.file_sha256(args.bench)})
    print_bench(results, 0)
    if args.net:
        _, net = read_jsonl(args.net)
        nw = {r["id"]: r["won"] for r in net}
        print("\n| boss | both win | net only | resolver only | neither |")
        print("|---|---:|---:|---:|---:|")
        for b in BOSSES:
            rs = [r for r in results if r["boss"] == b]
            c = [sum(1 for r in rs if (nw[r["id"]], r["won"]) == k)
                 for k in ((1, 1), (1, 0), (0, 1), (0, 0))]
            print(f"| {b} | " + " | ".join(map(str, c)) + " |")
    return 0


def copy_metrics(r, mon):
    """Per sampled play (a `bench --log` row): what the play did, turn by turn."""
    rows = r["rows"]
    turns = {}
    for x in rows:
        turns.setdefault(x[0], []).append(x)
    order = sorted(turns)
    taken, inc, cover, att_cover = [], [], [], []
    for a, b in zip(order, order[1:] + [None]):
        last = turns[a][-1]
        nxt = turns[b][0][1] if b is not None else (0 if not r["won"] else None)
        if nxt is None:
            continue
        taken.append(max(0, last[1] - nxt))
        inc.append(last[4])
        if last[4] > 0:
            c = min(last[2], last[4]) / last[4]
            cover.append(c)
            if last[4] >= 20:
                att_cover.append(c)
    dealt = {}
    prev = None
    first_kill = None
    for x in rows:
        hp = {mon[m]: h for m, h, _ in x[-1]}
        if prev is not None:
            for k, h in prev.items():
                dealt[k] = dealt.get(k, 0) + max(0, h - hp.get(k, 0))
            if first_kill is None and len(hp) < len(prev):
                first_kill = (x[0], [k for k in prev if k not in hp][0])
        prev = hp
    if r["won"] and prev:
        for k, h in prev.items():
            dealt[k] = dealt.get(k, 0) + h
    pots = [x[0] for x in rows if x[10] == "combat_potion"]
    ends = [x for x in rows if x[10] == "end_turn"]
    n_t = max(len(order), 1)
    return {
        "turns": len(order),
        "dealt_per_turn": sum(dealt.values()) / n_t,
        "taken_per_turn": sum(taken) / max(len(taken), 1),
        "incoming_per_turn": sum(inc) / max(len(inc), 1),
        "block_cover": float(np.mean(cover)) if cover else float("nan"),
        "block_cover_big": float(np.mean(att_cover)) if att_cover else float("nan"),
        "potions_used": len(pots),
        "potion_turn": float(np.mean(pots)) if pots else float("nan"),
        "osty_hp": float(np.mean([turns[t][0][6] for t in order])),
        "end_energy": float(np.mean([x[3] for x in ends])) if ends else float("nan"),
        "crusher_share": (dealt.get("CRUSHER", 0) / max(sum(dealt.values()), 1)
                          if "CRUSHER" in dealt else float("nan")),
        "first_kill": first_kill,
    }


PAIRED_KEYS = ("turns", "dealt_per_turn", "taken_per_turn", "incoming_per_turn", "block_cover",
               "block_cover_big", "potions_used", "potion_turn", "osty_hp", "end_energy",
               "crusher_share")


def cmd_paired(args):
    """Won vs lost sampled plays of the same fight (same entry state): the in-fight differences."""
    mon = _vocab("monsters")
    _, rs = read_jsonl(args.samples)
    by = {}
    for r in rs:
        by.setdefault(r["id"], []).append(r)
    print("| boss | mixed fights | " + " | ".join(PAIRED_KEYS) + " |")
    print("|---|---:|" + "---:|" * len(PAIRED_KEYS))
    crab = []
    for b in BOSSES:
        diffs = {k: [] for k in PAIRED_KEYS}
        won_m = {k: [] for k in PAIRED_KEYS}
        mixed = 0
        for i, cs in by.items():
            if cs[0]["boss"] != b:
                continue
            ms = [(c["won"], copy_metrics(c, mon)) for c in cs]
            if b == "KAISER_CRAB_BOSS":
                crab += ms
            w = [m for y, m in ms if y]
            lo = [m for y, m in ms if not y]
            if not w or not lo:
                continue
            mixed += 1
            for k in PAIRED_KEYS:
                a = [m[k] for m in w if not math.isnan(m[k])]
                c = [m[k] for m in lo if not math.isnan(m[k])]
                if a and c:
                    diffs[k].append(np.mean(a) - np.mean(c))
                    won_m[k].append(np.mean(a))
        cells = []
        for k in PAIRED_KEYS:
            if diffs[k]:
                d = np.asarray(diffs[k])
                se = d.std(ddof=1) / math.sqrt(len(d)) if len(d) > 1 else 0
                cells.append(f"{np.mean(won_m[k]):.2f} ({d.mean():+.2f}±{1.96 * se:.2f})")
            else:
                cells.append("-")
        print(f"| {b} | {mixed} | " + " | ".join(cells) + " |")
    print("\ncells: mean over mixed fights of the won plays' value (won minus lost, 95% CI)\n")
    print("Kaiser Crab, all sampled plays: first part killed\n")
    print("| outcome | plays | none killed | Crusher first | Rocket first | turn of first kill |")
    print("|---|---:|---:|---:|---:|---:|")
    for y in (1, 0):
        ms = [m for w, m in crab if w == y]
        fk = [m["first_kill"] for m in ms]
        none = sum(1 for f in fk if f is None)
        cr = sum(1 for f in fk if f and f[1] == "CRUSHER")
        ro = sum(1 for f in fk if f and f[1] == "ROCKET")
        t = [f[0] for f in fk if f]
        print(f"| {'won' if y else 'lost'} | {len(ms)} | {none} | {cr} | {ro} | "
              f"{np.mean(t) if t else float('nan'):.1f} |")
    return 0


C_CARDS, CARD_BYTES, C_HAND = 504, 6, 888


def played_card(row, slot):
    """The card vocab id in hand slot `slot`."""
    cid = u16(row, COMBAT_AT + C_HAND + 2 * slot)
    return u16(row, COMBAT_AT + C_CARDS + cid * CARD_BYTES)


def power_amount(row, name_id):
    """The summed amount of power `name_id` over all creatures (enemies' Sandpit, ...)."""
    total = 0
    for k in range(CREATURES):
        at = COMBAT_AT + C_CREATURES + k * CREATURE_BYTES
        if not row[at]:
            continue
        for q in range(12):
            pa = at + 12 + 4 * q
            if u16(row, pa) == name_id:
                total += i16(row, pa + 2)
    return total


def hand_of(row):
    """(card name, cost in hand) of each hand card."""
    names = _vocab("cards")
    out = []
    for slot in range(10):
        cid = u16(row, COMBAT_AT + C_HAND + 2 * slot)
        at = COMBAT_AT + C_CARDS + cid * CARD_BYTES
        kind = u16(row, at)
        if not kind:
            break
        out.append((names[kind], int(np.int8(row[at + 4]))))
    return out


def cmd_detail(args):
    """Greedy plays of the benchmark (CPU is enough) with the card of every play, the Sandpit
    counter and the cause of each death; per boss, won vs lost."""
    import torch

    from sts2 import HOLD, action_table

    cards, ctypes_ = _vocab("cards"), card_types()
    sandpit = _vocab("powers").index("SANDPIT_POWER") if "SANDPIT_POWER" in _vocab("powers") \
        else _vocab("powers").index("SANDPIT")
    table = action_table()
    kinds = EV_kinds()
    _, entries = read_jsonl(args.bench)
    entries = [e for e in entries if not args.boss or e["boss"] == args.boss]
    net = load_net(args.ckpt, args.weights)
    device = torch.device(args.device)
    env, rows = replay_prefixes(entries, 1)
    n = env.n
    open_ = np.ones(n, dtype=bool)
    won = np.zeros(n, dtype=int)
    plays = [[] for _ in range(n)]  # (turn, card name, type, energy before, sandpit)
    last = [None] * n
    while open_.any():
        idx = np.nonzero(open_)[0]
        action = policy_actions(net, env, idx, device)
        for j, i in enumerate(idx):
            row = env.obs[i]
            last[i] = (combat_row(row), power_amount(row, sandpit), hand_of(row))
            a = int(action[j])
            if kinds[a] == "hand_card":
                c = played_card(row, int(table[a, 1]))
                plays[i].append((last[i][0][0], cards[c], ctypes_[c], last[i][0][3], last[i][1]))
            else:
                plays[i].append((last[i][0][0], kinds[a], "-", last[i][0][3], last[i][1]))
        acts = np.full(n, HOLD, dtype=np.uint16)
        acts[idx] = action
        env.step(acts)
        for i in idx:
            if env.events[i, 1] > 0:
                won[i], open_[i] = 1, False
            elif env.done[i] or int(env.obs[i][H_PHASE]) != PHASE_COMBAT:
                open_[i] = False
    env.close()
    out = []
    for i, (e, _) in enumerate(rows):
        out.append({"id": e["id"], "boss": e["boss"], "won": int(won[i]), "plays": plays[i],
                    "last": last[i]})
    write_jsonl_gz(args.out, out, {"kind": "rln19-detail"})
    for b in BOSSES:
        for y in (1, 0):
            rs = [r for r in out if r["boss"] == b and r["won"] == y]
            if not rs:
                continue
            per = {}
            for r in rs:
                nt = max(1, max((p[0] for p in r["plays"]), default=1))
                for p in r["plays"]:
                    key = p[2] if p[2] != "-" else p[1]
                    per[key] = per.get(key, 0) + 1 / nt / len(rs)
            top = sorted(per.items(), key=lambda kv: -kv[1])
            print(f"{b} {'won' if y else 'lost'} {len(rs)}: per turn " +
                  ", ".join(f"{k} {v:.2f}" for k, v in top[:8]))
            names = {}
            for r in rs:
                for p in r["plays"]:
                    if p[2] != "-":
                        names[p[1]] = names.get(p[1], 0) + 1 / len(rs)
            print("   cards/fight: " + ", ".join(f"{k} {v:.1f}" for k, v in
                                                  sorted(names.items(), key=lambda kv: -kv[1])[:12]))
    return 0


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    h = sub.add_parser("harvest")
    h.add_argument("ckpt")
    h.add_argument("--weights", default="ema0.999")
    h.add_argument("--panel", default="DEV-1000")
    h.add_argument("--n", type=int)
    h.add_argument("--batch", type=int, default=500)
    h.add_argument("--device", default="cuda")
    h.add_argument("--gpu-window", action="store_true")
    h.add_argument("--out", required=True)
    b = sub.add_parser("build")
    b.add_argument("harvest", nargs="+")
    b.add_argument("--per", type=int, default=200)
    b.add_argument("--out", required=True)
    rh = sub.add_parser("rehash")
    rh.add_argument("bench")
    rh.add_argument("--batch", type=int, default=200)
    rh.add_argument("--out", required=True)
    r = sub.add_parser("bench")
    r.add_argument("ckpt")
    r.add_argument("--bench", required=True)
    r.add_argument("--weights", default="ema0.999")
    r.add_argument("--samples", type=int, default=0, help="0: greedy; K: K sampled plays per fight")
    r.add_argument("--temp", type=float, default=1.0)
    r.add_argument("--seed", type=int, default=0)
    r.add_argument("--batch", type=int, default=600)
    r.add_argument("--device", default="cuda")
    r.add_argument("--gpu-window", action="store_true")
    r.add_argument("--log", action="store_true", help="keep every decision's row")
    r.add_argument("--out")
    v = sub.add_parser("resolver")
    v.add_argument("--bench", required=True)
    v.add_argument("--net", help="a greedy `bench` result to pair with")
    v.add_argument("--out")
    q = sub.add_parser("paired")
    q.add_argument("--samples", required=True, help="a `bench --samples K --log` result")
    d = sub.add_parser("detail")
    d.add_argument("ckpt")
    d.add_argument("--bench", required=True)
    d.add_argument("--boss")
    d.add_argument("--weights", default="ema0.999")
    d.add_argument("--device", default="cpu")
    d.add_argument("--out", required=True)
    a = sub.add_parser("analyze")
    a.add_argument("--harvest", required=True, nargs="+")
    args = p.parse_args()
    return {"harvest": cmd_harvest, "build": cmd_build, "rehash": cmd_rehash, "bench": cmd_bench,
            "analyze": cmd_analyze, "resolver": cmd_resolver, "paired": cmd_paired, "detail": cmd_detail}[args.cmd](args)


if __name__ == "__main__":
    raise SystemExit(main())
