"""CB1: the combat bot's training fight starts (`docs/research/cb1-combat-bot.md` §2).

    python python/cb1_data.py build [--seed 0] [--mix b]  # -> ~/.cache/spirebot-data/cb1/train[_b].jsonl

Gym records (`sts2_rl::human::HumanStart`) from four sources, the NAZ3 benchmark held out:

- **own**: NAZ1's harvest of NECRO5's A10 runs (`own_train`, `own_train2`: run seeds 300M-300.006M
  and 320M-320.03M; the benchmark's runs are 310M+, `own_test`, never read here), subsampled;
- **pert**: NAZ1's perturbations of those starts (a card added, upgraded, removed; HP 25-100%;
  another encounter of the same act model and room);
- **human**: the 23 gym players' fights (>50% A10 players; RLN43's panel players are not in them)
  and their perturbations;
- **proc**: procedural decks so the bot is not tied to one deck style: the A10 starter deck with
  0-4 Strikes/Defends removed and 2-24 cards added (by act), drawn from that act's cards seen in
  the data with weights count^0.5 (flatter than the data), a random upgrade rate, 1-12 relics and
  0-3 potions the same way, HP 30-100% of a real start's max HP, on a real start's encounter,
  act model, floor and gold. Only content seen in loadable records is used.

Any record whose run seed or id is in the benchmark is dropped (there are none by construction;
the check guards the file). Human-derived data: never committed.
"""

import argparse
import collections
import json
import os
import pathlib
import sys

import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

NAZ1 = pathlib.Path(os.path.expanduser("~/.cache/spirebot-data/naz1"))
BENCH = pathlib.Path(os.path.expanduser("~/.cache/spirebot-data/naz3/bench.jsonl"))
DATA = pathlib.Path(os.path.expanduser("~/.cache/spirebot-data/cb1"))
ENGINE_KEYS = ("act", "act_id", "encounter", "floor", "hp", "max_hp", "gold", "deck", "relics",
               "potion_slots", "potions")
STARTER = [["STRIKE_NECROBINDER", 0]] * 4 + [["DEFEND_NECROBINDER", 0]] * 4 + [
    ["BODYGUARD", 0], ["UNLEASH", 0], ["ASCENDERS_BANE", 0]]
STARTER_IDS = {c for c, _ in STARTER}
BASE_RELIC = "BOUND_PHYLACTERY"
ADD = {0: (2, 10), 1: (6, 18), 2: (10, 24)}  # cards added, by act (inclusive)
RELICS = {0: (1, 4), 1: (3, 8), 2: (5, 12)}  # extra relics, by act
SHARE = {"own": 160_000, "pert": 80_000, "human": None, "proc": 120_000}  # None: all
# Stage 1b (`--mix b`): every own start, fewer procedural decks (they die ~98% at bosses).
SHARE_B = {"own": None, "pert": 80_000, "human": None, "proc": 40_000}
# Lane h (`--mix h`): human decks first: all 138k human starts (~75%), own + procedural ~25%.
SHARE_H = {"own": 30_000, "pert": 6_000, "human": None, "proc": 10_000}


def room_of(r):
    e = r["encounter"]
    return "boss" if e.endswith("_BOSS") else "elite" if e.endswith("_ELITE") else "hallway"


def read_jsonl(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def proc_starts(templates, n, rng):
    """`n` procedural starts on the encounters of `templates` (see the module docs)."""
    by_act = collections.defaultdict(list)
    for r in templates:
        by_act[r["act"]].append(r)
    pools = {}
    for act, rs in by_act.items():
        cards = collections.Counter(c for r in rs for c, _ in r["deck"] if c not in STARTER_IDS)
        relics = collections.Counter(x for r in rs for x in r["relics"] if x != BASE_RELIC)
        pots = collections.Counter(x for r in rs for x in r["potions"])

        def pool(c):
            k = sorted(c)
            w = np.sqrt(np.array([c[x] for x in k], dtype=float))
            return k, w / w.sum()
        pools[act] = pool(cards), pool(relics), pool(pots)
    out = []
    for i in range(n):
        t = templates[rng.integers(len(templates))]
        act = t["act"]
        (ck, cw), (rk, rw), (pk, pw) = pools[act]
        deck = [list(c) for c in STARTER]
        for _ in range(rng.integers(0, 5)):  # remove Strikes/Defends
            basics = [j for j, (c, _) in enumerate(deck) if c in ("STRIKE_NECROBINDER",
                                                                  "DEFEND_NECROBINDER")]
            if basics:
                deck.pop(basics[rng.integers(len(basics))])
        lo, hi = ADD[act]
        deck += [[str(c), 0] for c in rng.choice(ck, rng.integers(lo, hi + 1), p=cw)]
        up = rng.uniform(0.0, 0.15 + 0.25 * act)
        for c in deck:
            if c[0] != "ASCENDERS_BANE" and rng.random() < up:
                c[1] = 1
        lo, hi = RELICS[act]
        k = min(len(rk), int(rng.integers(lo, hi + 1)))
        relics = [BASE_RELIC] + [str(x) for x in rng.choice(rk, k, replace=False, p=rw)]
        slots = t["potion_slots"]
        np_ = min(slots, int(rng.integers(0, slots + 1)), len(pk))
        pots = [str(x) for x in rng.choice(pk, np_, replace=False, p=pw)] if np_ else []
        mx = t["max_hp"]
        hp = max(1, int(round(mx * rng.uniform(0.3, 1.0))))
        out.append(dict(act=act, act_id=t["act_id"], encounter=t["encounter"], floor=t["floor"],
                        hp=hp, max_hp=mx, gold=t["gold"], deck=deck, relics=relics,
                        potion_slots=slots, potions=pots, src="proc", id=f"proc{i}"))
    return out


def try_load(env, path):
    try:
        return env.load_gym(path)
    except Exception:  # noqa: BLE001 (a parse error: the line is reported as dropped)
        return -1


def loadable(recs):
    """The records the engine's gym parser accepts (a bad line is dropped with its reason)."""
    from sts2 import E2E, Sts2RunEnvs

    tmp = DATA / f"check-{os.getpid()}.jsonl"
    keep, bad = [], collections.Counter()
    env = Sts2RunEnvs(1, seed_range=(0, 1), ascension=10, threads=1, **E2E)
    try:
        for k in range(0, len(recs), 20_000):
            chunk = recs[k:k + 20_000]
            tmp.write_text("".join(json.dumps({x: r[x] for x in ENGINE_KEYS}) + "\n"
                                   for r in chunk))
            if try_load(env, tmp) == len(chunk):
                keep += chunk
                continue
            for r in chunk:  # find the bad lines one by one
                tmp.write_text(json.dumps({x: r[x] for x in ENGINE_KEYS}) + "\n")
                if try_load(env, tmp) == 1:
                    keep.append(r)
                else:
                    bad[r["src"]] += 1
    finally:
        env.close()
        tmp.unlink(missing_ok=True)
    return keep, bad


def cmd_build(args):
    rng = np.random.default_rng(args.seed)
    bench = read_jsonl(BENCH)
    b_ids = {r["id"] for r in bench}
    b_seeds = {r["seed"] for r in bench}
    own = read_jsonl(NAZ1 / "own_train.jsonl") + read_jsonl(NAZ1 / "own_train2.jsonl")
    pert = read_jsonl(NAZ1 / "pert_train.jsonl")
    human = []
    for f in ("human_gym", "human_boss_x", "pert_human", "pert_human_boss_x"):
        human += read_jsonl(NAZ1 / f"{f}.jsonl")
    for r in human:
        r["src"] = "human" if r["src"] == "human" else "human_" + r["src"]
    for r in pert:
        r["src"] = "own_" + r["src"]

    def sub(rs, n):
        if n is None or n >= len(rs):
            return rs
        return [rs[i] for i in sorted(rng.choice(len(rs), n, replace=False))]

    share = {"a": SHARE, "b": SHARE_B, "h": SHARE_H}[args.mix]
    own, pert = sub(own, share["own"]), sub(pert, share["pert"])
    proc = proc_starts(own + human, share["proc"], rng)
    recs = own + pert + human + proc
    recs = [r for r in recs if r.get("id") not in b_ids and r.get("seed") not in b_seeds
            and r.get("base") not in b_ids]
    recs, bad = loadable(recs)
    order = rng.permutation(len(recs))
    DATA.mkdir(parents=True, exist_ok=True)
    out = DATA / ("train.jsonl" if args.mix == "a" else f"train_{args.mix}.jsonl")
    with open(out, "w") as f:
        for i in order:
            r = recs[i]
            f.write(json.dumps({**{k: r[k] for k in ENGINE_KEYS}, "src": r["src"],
                                "id": r.get("id", "")}) + "\n")
    c = collections.Counter((r["act"], room_of(r)) for r in recs)
    s = collections.Counter(r["src"].split("_")[0] for r in recs)
    print(f"{out}: {len(recs)} starts; dropped {dict(bad)}")
    print(f"  by source {dict(s)}")
    print(f"  by cell {sorted(c.items())}")
    return 0


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    b = sub.add_parser("build")
    b.add_argument("--seed", type=int, default=0)
    b.add_argument("--mix", default="a", choices=("a", "b", "h"))
    args = p.parse_args()
    return {"build": cmd_build}[args.cmd](args)


if __name__ == "__main__":
    raise SystemExit(main())
