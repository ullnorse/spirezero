"""NAZ1: fight starts and their rollout labels for the fight-outcome model.

    python python/naz1_rollouts.py harvest CKPT --first S --count N --out F   # own-run fight starts
    python python/naz1_rollouts.py label CKPT --starts F --reps R --seed S --out F.npz
    python python/naz1_rollouts.py sets        # perturbation sets: pert_train, cf_base*
    python python/naz1_rollouts.py dense       # pert_human: every human gym start x 5 kinds

**Starts** are gym records (`sts2_rl::human::HumanStart`, one JSON line each), plus the fields
`src` (human, own, a perturbation kind), `id` and, for a perturbation, `base` (the id of the start
it changes) and `change`.

**harvest:** NECRO5's policy plays whole A10 runs on seeds `first..first + count`, sampling its
actions (the states a planner meets); every combat room's first decision is recorded with
`Sts2RunEnvs.start_record` (the engine's run state as a gym record: relic counters and card
enchantments are dropped, as in the human records).

**label:** every start is played `reps` times, greedily, by the policy in the combat gym
(sequential: rollout `k` of the file is built on seed `seed + k`, so each rollout has fresh
streams for the run, the shuffle and the monsters). Per rollout: HP lost (entry HP minus HP at the
first decision after the fight; a death or a cap loses the entry HP), died, potions used, and
`bad` (the env did not hold the record it should: excluded; the run aborts above 0.1%).
The env hands records out in env order as episodes end (`RunEnvs::assign`), which this replays;
each new episode's encounter and deck size are checked against the record it should hold.

**sets:** the counterfactuals a planner asks about, per base start: one card added (drawn from
the cards seen in the data, weighted by count), one card upgraded, one card removed, HP drawn in
25-100% of max HP, and another encounter of the same act model and room.

Data lives in `~/.cache/spirebot-data/naz1/` (human-derived: never committed).
"""

import argparse
import collections
import json
import os
import pathlib
import sys
import time

import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

DATA = pathlib.Path(os.path.expanduser("~/.cache/spirebot-data/naz1"))
ENGINE_KEYS = ("act", "act_id", "encounter", "floor", "hp", "max_hp", "gold", "deck", "relics",
               "potion_slots", "potions")
H_PHASE, H_HP, PHASE_COMBAT = 3, 8, 3
BELT_AT, BELT_SLOTS = 480, 6
GPU_CHUNK = 4096


def engine_line(r):
    return json.dumps({k: r[k] for k in ENGINE_KEYS})


def read_jsonl(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def write_jsonl(path, recs):
    tmp = pathlib.Path(str(path) + ".tmp")
    with open(tmp, "w") as f:
        for r in recs:
            f.write(json.dumps(r) + "\n")
    tmp.rename(path)


def net_and_dev(ckpt, weights):
    import torch

    import e2e_eval as EV

    torch.set_num_threads(2)
    dev = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    if dev.type == "cuda":  # leave room for a training job on the same GPU
        torch.cuda.set_per_process_memory_fraction(float(os.environ.get("NAZ1_GPU_FRAC", "0.6")))
    return EV.load_policy(ckpt, weights, dev), dev


def actions(net, env, idx, dev, sample=False, gen=None):
    import torch

    import rln36_boss as B

    out = []
    for k in range(0, len(idx), GPU_CHUNK):
        lg = B.logits_of(net, env, idx[k:k + GPU_CHUNK], dev)
        if sample:
            a = torch.multinomial(torch.softmax(lg, -1), 1, generator=gen).squeeze(1)
        else:
            a = lg.argmax(-1)
        out.append(a.cpu().numpy())
    return np.concatenate(out) if out else np.zeros(0, dtype=np.int64)


def i16(row, at):
    return int(np.int16(int(row[at]) | int(row[at + 1]) << 8))


def belt_count(row):
    return sum(1 for k in range(BELT_SLOTS) if row[BELT_AT + 2 * k] or row[BELT_AT + 2 * k + 1])


# ---------------------------------------------------------------------------------------------


def cmd_harvest(args):
    import torch

    import rln36_boss as B
    from sts2 import HOLD

    net, dev = net_and_dev(args.ckpt, args.weights)
    gen = torch.Generator(device=dev)
    gen.manual_seed(args.first)
    out, t0 = [], time.time()
    for b0 in range(args.first, args.first + args.count, args.batch):
        seeds = np.arange(b0, min(b0 + args.batch, args.first + args.count), dtype=np.uint64)
        n = len(seeds)
        env = B.make_env(net, n, seeds=seeds)
        try:
            live = np.ones(n, dtype=bool)
            was = np.zeros(n, dtype=bool)
            while live.any():
                idx = np.nonzero(live)[0]
                phase = env.obs[idx, H_PHASE] == PHASE_COMBAT
                for j in idx[phase & ~was[idx]]:
                    line = env.start_record(int(j))
                    if line is not None:
                        r = json.loads(line)
                        r.update(src="own", seed=int(seeds[j]))
                        out.append(r)
                was[idx] = phase
                a = actions(net, env, idx, dev, sample=True, gen=gen)
                acts = np.full(n, HOLD, dtype=np.uint16)
                acts[idx] = a
                env.step(acts)
                for j in idx[env.done[idx].astype(bool)]:
                    live[j] = False
        finally:
            env.close()
        print(f"seeds {b0}+{n}: {len(out)} starts, {time.time() - t0:.0f}s", flush=True)
    for k, r in enumerate(out):
        r["id"] = f"own{r['seed']}-{r['floor']}"
    write_jsonl(args.out, out)
    c = collections.Counter((r["act"], r["encounter"].rsplit("_", 1)[-1]) for r in out)
    print(json.dumps(sorted((f"{a}:{k}", v) for (a, k), v in c.items())))
    return 0


# ---------------------------------------------------------------------------------------------


def label(net, dev, recs, reps, seed, n_envs):
    """(hp_lost, died, potions) arrays of shape (len(recs), reps); see the module docs."""
    import rln36_boss as B
    from sts2 import HOLD
    from sts2.env import RUN_ENDS

    total = len(recs) * reps
    DATA.mkdir(parents=True, exist_ok=True)
    tmp = DATA / f"label-{os.getpid()}.jsonl"
    tmp.write_text("".join((engine_line(r) + "\n") * reps for r in recs))
    n = min(n_envs, total)
    hp_lost = np.zeros(total, dtype=np.int16)
    died = np.zeros(total, dtype=np.uint8)
    potions = np.zeros(total, dtype=np.int8)
    entry_belt = np.zeros(total, dtype=np.int8)
    env = B.make_env(net, n, seed_range=(0, 1), cycle=True)
    bad = np.zeros(total, dtype=bool)
    try:
        assert env.load_gym(tmp) == total
        env.set_gym(first=0, slots=n, sequential=True, seed=seed)
        env.reset_all()
        holds = np.arange(n)  # the rollout each env plays
        nxt = n

        def check(i):
            k = holds[i]
            line = env.start_record(int(i))
            r = recs[k // reps]
            got = json.loads(line) if line else {}
            if got.get("encounter") != r["encounter"] or len(got.get("deck", ())) != len(r["deck"]):
                bad[k] = True
                if bad.sum() <= 5:
                    print(f"  mismatch: rollout {k} of {r.get('id')} ({r['encounter']}, "
                          f"{len(r['deck'])} cards) holds {got.get('encounter')} "
                          f"{len(got.get('deck', ()))} cards", flush=True)
            entry_belt[k] = belt_count(env.obs[i])

        for i in range(n):
            check(i)
        live = np.ones(n, dtype=bool)
        steps, t0 = 0, time.time()
        while live.any():
            idx = np.nonzero(live)[0]
            a = actions(net, env, idx, dev)
            acts = np.full(n, HOLD, dtype=np.uint16)
            acts[idx] = a
            env.step(acts)
            steps += 1
            for i in idx[env.done[idx].astype(bool)]:  # ascending: the env's assign order
                k = holds[i]
                end = RUN_ENDS[env.end[i]]
                hp0 = recs[k // reps]["hp"]
                if end in ("gym_end", "victory"):
                    fo = env.final_obs[i]
                    hp_lost[k] = hp0 - i16(fo, H_HP)
                    potions[k] = entry_belt[k] - belt_count(fo)
                else:
                    hp_lost[k], died[k] = hp0, 1
                if nxt < total:
                    holds[i] = nxt
                    nxt += 1
                    check(i)
                else:
                    live[i] = False
            if steps % 500 == 0:
                done = nxt - live.sum()
                print(f"  step {steps}: {done}/{total} rollouts, {done / (time.time() - t0):.0f}/s",
                      flush=True)
    finally:
        env.close()
        tmp.unlink(missing_ok=True)
    if bad.mean() > 0.001:
        raise SystemExit(f"{bad.sum()} rollouts did not hold the record they should")
    shape = (len(recs), reps)
    return (hp_lost.reshape(shape), died.reshape(shape), potions.reshape(shape),
            bad.reshape(shape))


def cmd_label(args):
    net, dev = net_and_dev(args.ckpt, args.weights)
    recs = read_jsonl(args.starts)
    if args.limit:
        recs = recs[:args.limit]
    t0 = time.time()
    out = pathlib.Path(args.out)
    parts = []
    for c0 in range(0, len(recs), args.chunk):
        part = out.with_suffix(f".part{c0}.npz")
        if part.exists():
            parts.append(part)
            continue
        chunk = recs[c0:c0 + args.chunk]
        h, d, p, b = label(net, dev, chunk, args.reps, args.seed + c0 * args.reps, args.envs)
        np.savez(part, hp_lost=h, died=d, potions=p, bad=b)
        parts.append(part)
        el = time.time() - t0
        print(f"{c0 + len(chunk)}/{len(recs)} starts x {args.reps}: {el:.0f}s "
              f"(deaths {d.mean():.3f}, mean loss {h.mean():.2f})", flush=True)
    z = [np.load(p) for p in parts]
    np.savez(out, ids=np.array([r["id"] for r in recs]), seed=args.seed, reps=args.reps,
             ckpt=str(args.ckpt), weights=args.weights,
             **{k: np.concatenate([x[k] for x in z]) for k in ("hp_lost", "died", "potions", "bad")})
    for p in parts:
        p.unlink()
    print(f"wrote {out}: {len(recs)} starts x {args.reps} in {time.time() - t0:.0f}s")
    return 0


# ---------------------------------------------------------------------------------------------


class Perturber:
    """Single changes of a start (module docs), drawn from a pool's cards and encounters."""

    def __init__(self, pool, seed):
        from sts2 import ffi

        self.lib = ffi.load()
        self.rng = np.random.default_rng(seed)
        counts = collections.Counter(c[0] for r in pool for c in r["deck"]
                                     if not c[0].startswith(("STRIKE_", "DEFEND_")))
        self.cards = sorted(counts)
        w = np.array([counts[c] for c in self.cards], dtype=np.float64)
        self.w = w / w.sum()
        encs = collections.defaultdict(set)
        for r in pool:
            encs[(r["act_id"], room_of(r))].add(r["encounter"])
        self.encs = {k: sorted(v) for k, v in encs.items()}

    def variant(self, r, kind):
        """The changed start (engine keys + `src`, `id`, `base`, `change`), or None."""
        from sts2 import ffi

        rng = self.rng
        v = {k: r[k] for k in ENGINE_KEYS}
        v["deck"] = [list(c) for c in r["deck"]]
        if kind == "add":
            c = self.cards[rng.choice(len(self.cards), p=self.w)]
            v["deck"].append([c, int(rng.random() < 0.25)])
            change = f"+{c}"
        elif kind == "upgrade":
            opts = [i for i, c in enumerate(v["deck"]) if c[1] == 0]
            if not opts:
                return None
            i = opts[rng.integers(len(opts))]
            v["deck"][i][1] = 1
            change = f"^{v['deck'][i][0]}"
        elif kind == "remove":
            if len(v["deck"]) < 6:
                return None
            i = int(rng.integers(len(v["deck"])))
            change = f"-{v['deck'][i][0]}"
            del v["deck"][i]
        elif kind == "hp":
            v["hp"] = int(max(1, round(v["max_hp"] * rng.uniform(0.25, 1.0))))
            change = f"hp{v['hp']}"
        elif kind == "encounter":
            opts = [e for e in self.encs.get((v["act_id"], room_of(r)), []) if e != v["encounter"]]
            if not opts:
                return None
            v["encounter"] = opts[rng.integers(len(opts))]
            change = f"@{v['encounter']}"
        else:
            raise SystemExit(f"kind {kind}")
        if ffi.human_check(self.lib, engine_line(v)) is not None:
            return None
        v.update(src=f"p_{kind}", id=f"{r['id']}/{kind}", base=r["id"], change=change)
        return v


def room_of(r):
    e = r["encounter"]
    return "boss" if e.endswith("_BOSS") else "elite" if e.endswith("_ELITE") else "hallway"


def sample_bases(recs, n, rng, fight_share=0.6):
    """`n` starts, `fight_share` of them elites and bosses (as many as there are)."""
    big = [r for r in recs if room_of(r) != "hallway"]
    hall = [r for r in recs if room_of(r) == "hallway"]
    nb = min(len(big), int(n * fight_share))
    pick = [big[i] for i in rng.choice(len(big), nb, replace=False)]
    pick += [hall[i] for i in rng.choice(len(hall), min(len(hall), n - nb), replace=False)]
    return pick


def cmd_dense(args):
    """`pert_human`: one variant of each kind for every human gym start (the human-like states
    are the scarcest; `--seed` differs from `sets`, so the draws are new)."""
    gym, own = read_jsonl(DATA / "human_gym.jsonl"), read_jsonl(DATA / "own_train.jsonl")
    pert = Perturber(gym + own, args.seed)
    kinds = ("add", "upgrade", "remove", "hp", "encounter")
    out = [v for r in gym for k in kinds if (v := pert.variant(r, k)) is not None]
    for v in out:
        v["id"] += "/d"
    write_jsonl(DATA / "pert_human.jsonl", out)
    print(f"pert_human: {len(out)} variants of {len(gym)} starts")
    return 0


def cmd_sets(args):
    """The perturbation sets: `pert_train` (all kinds, on training bases) and the held-out
    counterfactual pairs `cf_base` + `cf_base_cf_<kind>` (add, upgrade, remove), aligned row by
    row, so labelling them with the same seed pairs their rollouts' seeds."""
    rng = np.random.default_rng(args.seed)
    gym, own = read_jsonl(DATA / "human_gym.jsonl"), read_jsonl(DATA / "own_train.jsonl")
    pert = Perturber(gym + own, args.seed)
    bases = sample_bases(gym, args.train_bases // 2, rng) + sample_bases(own, args.train_bases // 2, rng)
    kinds = ("add", "upgrade", "remove", "hp", "encounter")
    out = [v for r in bases for k in kinds if (v := pert.variant(r, k)) is not None]
    write_jsonl(DATA / "pert_train.jsonl", out)
    print(f"pert_train: {len(out)} variants of {len(bases)} bases")
    panel, own_t = read_jsonl(DATA / "human_panel.jsonl"), read_jsonl(DATA / "own_test.jsonl")
    bases = sample_bases(panel, args.cf_bases // 2, rng, 0.67) + sample_bases(own_t, args.cf_bases // 2, rng, 0.67)
    cf_kinds = ("add", "upgrade", "remove")
    rows = []
    for r in bases:
        vs = [pert.variant(r, k) for k in cf_kinds]
        if all(v is not None for v in vs):
            rows.append((r, vs))
    write_jsonl(DATA / "cf_base.jsonl", [r for r, _ in rows])
    for j, k in enumerate(cf_kinds):
        write_jsonl(DATA / f"cf_base_cf_{k}.jsonl", [vs[j] for _, vs in rows])
    print(f"cf: {len(rows)} bases x {len(cf_kinds)} kinds")
    return 0


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    h = sub.add_parser("harvest")
    h.add_argument("ckpt")
    h.add_argument("--weights", default="ema0.999")
    h.add_argument("--first", type=int, required=True)
    h.add_argument("--count", type=int, required=True)
    h.add_argument("--batch", type=int, default=2048)
    h.add_argument("--out", required=True)
    lb = sub.add_parser("label")
    lb.add_argument("ckpt")
    lb.add_argument("--weights", default="ema0.999")
    lb.add_argument("--starts", required=True)
    lb.add_argument("--reps", type=int, default=16)
    lb.add_argument("--seed", type=int, required=True)
    lb.add_argument("--envs", type=int, default=4096)
    lb.add_argument("--chunk", type=int, default=4000)
    lb.add_argument("--limit", type=int, default=0)
    lb.add_argument("--out", required=True)
    st = sub.add_parser("sets")
    st.add_argument("--train-bases", type=int, default=16000)
    st.add_argument("--cf-bases", type=int, default=1600)
    st.add_argument("--seed", type=int, default=1)
    dn = sub.add_parser("dense")
    dn.add_argument("--seed", type=int, default=2)
    args = p.parse_args()
    return {"dense": cmd_dense, "harvest": cmd_harvest, "label": cmd_label, "sets": cmd_sets}[args.cmd](args)


if __name__ == "__main__":
    raise SystemExit(main())
