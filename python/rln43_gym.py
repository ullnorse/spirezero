"""RLN43: the combat gym's human fight starts and the paired fight benchmark.

    python python/rln43_gym.py build                 # runs -> gym starts + held-out panel, counts
    python python/rln43_gym.py bench CKPT [--weights ema0.999] [--out F] [--own-panel P]
    python python/rln43_gym.py compare A.json B.json [C.json ...]   # paired, per act and room

Data stays local: everything is read from and written to `~/.cache/spirebot-data/rln43/`
(`DATA`, from RLN41's `runs.jsonl.gz`); nothing derived from the dump is committed.

**A start** (one JSON line, `sts2_rl::human::HumanStart`): one combat room of a strong player's
A10 Necrobinder run (RLN41's 37 players), as entered: `act` (0-2), `act_id` (the run's act model),
`encounter`, `floor` (the file's count, Neow is 1), `hp`, `max_hp`, `gold`, `deck` [[card,
upgrade]], `relics`, `potion_slots`, `potions`; plus, for the bench and the counts, `player`,
`run`, `room` (monster, elite, boss), `hp_lost` and `died` (the human's outcome) and `turns`.

**State at entry** (approximations in `docs/research/rln43-combat-gym.md`):
- deck and relics as RLN41 rebuilds them before the room (`rln41_dataset.Deck`, `relics_at`);
- HP = the file's post-room HP + the room's `damage_taken` - `hp_healed`, max HP = post-room max
  HP - `max_hp_gained` + `max_hp_lost`; gold = post-room gold - gained + lost + stolen;
- the belt is rebuilt from the potions gained (`potion_choices` picked, `bought_potions`) less
  those used and discarded, in gain order, capped at the slots (2 at A10, +2 `POTION_BELT`, +1
  `PHIAL_HOLSTER`, +4 `ALCHEMICAL_COFFER`); a potion the engine doesn't port is left out;
- relic counters start fresh; cards have no enchantment or counter.

**Drops:** an entry the engine can't build (`ffi.human_check`: an unported card, relic or
encounter, an encounter outside its act's list) is dropped and counted by reason.

**Split:** by player. RLN41's 6 held-out players, then more in a fixed order until the panel holds
>= `PANEL_BOSSES` bosses and >= `PANEL_ELITES` elites; their starts are the panel
(`panel.jsonl`), every other player's are the gym (`gym.jsonl`).

**Bench** (`bench`): every panel fight once, on seed `BENCH_SEED + k` (the same for every pin, so
two pins are paired on the fight and its streams), played greedily by the net until the fight
ends; then RLN39's own-entry panel (`rln39_fightbench.play`). Per act and room: deaths per 100
and mean HP lost (% of max HP, a death counts its entry HP), with the humans' own outcome on the
same fights as the reference. `compare`: A minus B per cell, deaths with Newcombe's paired 95% CI
and HP lost with a paired t interval.
"""

import argparse
import collections
import gzip
import json
import math
import os
import pathlib
import sys
import time

import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import rln41_dataset as D  # noqa: E402

DATA = pathlib.Path(os.path.expanduser("~/.cache/spirebot-data/rln43"))
GYM = DATA / "gym.jsonl"
PANEL = DATA / "panel.jsonl"
COUNTS = DATA / "counts.json"
PANEL_BOSSES, PANEL_ELITES = 1000, 1000
BENCH_SEED = 43_000_000
ROOMS = ("monster", "elite", "boss")
BASE_SLOTS = 2  # A10 (`TightBelt`): 3 - 1
SLOT_RELICS = {"POTION_BELT": 2, "PHIAL_HOLSTER": 1, "ALCHEMICAL_COFFER": 4}
STEP_CAP = 4000
CHUNK = 512


# ---------------------------------------------------------------------------------------------
# Build


class Belt:
    """The potion belt, rebuilt from gains, uses and discards (in gain order)."""

    def __init__(self):
        self.potions = []

    def apply(self, s, slots):
        for p in s.get("potion_used", []) + s.get("potion_discarded", []):
            if D.bare(p) in self.potions:
                self.potions.remove(D.bare(p))
        gained = [D.bare(c["choice"]) for c in s.get("potion_choices", []) if c.get("was_picked")]
        gained += [D.bare(p) for p in s.get("bought_potions", [])]
        for p in gained:
            if len(self.potions) < slots:
                self.potions.append(p)


def slots_of(relics):
    return BASE_SLOTS + sum(SLOT_RELICS.get(r, 0) for r in relics)


def entries(x):
    """The combat rooms of one run (dicts, see the module docs)."""
    run = x["run"]
    acts = [D.bare(a) for a in run["acts"]]
    deck, belt = D.Deck(), Belt()
    timeline = D.relic_timeline(run)
    lost = not x["win"]
    killer = D.bare(run.get("killed_by_encounter") or "")
    out = []
    floor = 0
    hist = run["map_point_history"]
    last = (len(hist) - 1, len(hist[-1]) - 1) if hist and hist[-1] else None
    for act_i, act in enumerate(hist):
        for j, e in enumerate(act):
            floor += 1
            stats = e["player_stats"][0] if e["player_stats"] else None
            if stats is None:
                continue
            held = D.relics_at(timeline, floor)
            slots = slots_of(held)
            for room in e.get("rooms") or []:
                rt = room.get("room_type", "")
                mid = room.get("model_id") or ""
                if rt not in ROOMS or not mid.startswith("ENCOUNTER."):
                    continue
                max_hp = stats["max_hp"] - stats.get("max_hp_gained", 0) + stats.get("max_hp_lost", 0)
                hp = stats["current_hp"] + stats.get("damage_taken", 0) - stats.get("hp_healed", 0)
                died = bool(lost and (act_i, j) == last and D.bare(mid) == killer)
                gold = (stats.get("current_gold", 0) - stats.get("gold_gained", 0)
                        + stats.get("gold_lost", 0) + stats.get("gold_stolen", 0))
                out.append(dict(
                    act=act_i, act_id=acts[act_i] if act_i < len(acts) else "", encounter=D.bare(mid),
                    floor=floor, hp=int(max(1, min(max_hp, hp))), max_hp=int(max(1, max_hp)),
                    gold=int(max(0, gold)), deck=[[c, u] for c, u in deck.cards],
                    relics=list(held), potion_slots=slots, potions=list(belt.potions[:slots]),
                    player=x["player"], run=x["id"], room=rt,
                    hp_lost=int(min(stats.get("damage_taken", 0), max(1, hp))), died=died,
                    turns=int(room.get("turns_taken", 0))))
                break  # one fight per point
            deck.apply(stats)
            belt.apply(stats, slots)
    return out


ENGINE_KEYS = ("act", "act_id", "encounter", "floor", "hp", "max_hp", "gold", "deck", "relics",
               "potion_slots", "potions")


def engine_line(r):
    return json.dumps({k: r[k] for k in ENGINE_KEYS})


def check_all(lib, recs):
    """Keeps the records the engine builds; a record failing only on a potion retries without
    it (counted). Returns (kept, drop reasons Counter, potions left out Counter)."""
    from sts2 import ffi

    kept, why, potions = [], collections.Counter(), collections.Counter()
    for r in recs:
        err = ffi.human_check(lib, engine_line(r))
        while err is not None and err.startswith("potion "):
            p = err.split()[1]
            potions[p] += 1
            r["potions"] = [q for q in r["potions"] if q != p]
            err = ffi.human_check(lib, engine_line(r))
        if err is None:
            kept.append(r)
        else:
            kind = err.split(":")[0].split()[0]
            what = err.split()[1] if kind in ("card", "relic", "encounter") else ""
            why[f"{kind} {what}".strip()] += 1
    return kept, why, potions


def split_players(recs, heldout):
    """RLN41's held-out players, then more (a fixed order) until the panel's boss and elite counts
    are reached."""
    rng = np.random.default_rng(43)
    players = sorted({r["player"] for r in recs})
    rest = [p for p in rng.permutation(players).tolist() if p not in heldout]
    panel = set(heldout)
    count = lambda room: sum(r["room"] == room and r["player"] in panel for r in recs)  # noqa: E731
    while (count("boss") < PANEL_BOSSES or count("elite") < PANEL_ELITES) and rest:
        panel.add(rest.pop(0))
    return panel


def table(recs, title):
    print(f"\n{title}: {len(recs)} starts")
    print("| act | monster | elite | boss |\n|---|---:|---:|---:|")
    for a in range(3):
        c = collections.Counter(r["room"] for r in recs if r["act"] == a)
        print(f"| {a + 1} | {c['monster']} | {c['elite']} | {c['boss']} |")


def cmd_build(args):
    from sts2 import ffi

    lib = ffi.load()
    if not ffi.has_gym(lib):
        raise SystemExit("the gym needs an ABI-15 library: cargo build --profile ffi -p sts2-ffi")
    t0 = time.time()
    runs = D.load_runs()
    recs = [r for x in runs for r in entries(x)]
    n_all = len(recs)
    kept, why, potions = check_all(lib, recs)
    heldout = D.heldout_players({x["player"] for x in runs})
    panel_players = split_players(kept, heldout)
    panel = [r for r in kept if r["player"] in panel_players]
    gym = [r for r in kept if r["player"] not in panel_players]
    DATA.mkdir(parents=True, exist_ok=True)
    for path, rs in ((GYM, gym), (PANEL, panel)):
        with open(path, "w") as f:
            for r in rs:
                f.write(json.dumps(r) + "\n")
    by_room = collections.Counter(r["room"] for r in recs)
    drop_room = {room: by_room[room] - sum(r["room"] == room for r in kept) for room in ROOMS}
    counts = dict(runs=len(runs), entries=n_all, kept=len(kept), dropped=n_all - len(kept),
                  drop_pct=100 * (n_all - len(kept)) / max(1, n_all),
                  drop_by_room={k: [v, by_room[k]] for k, v in drop_room.items()},
                  drop_reasons=dict(why.most_common()), potions_left_out=dict(potions.most_common()),
                  panel_players=len(panel_players), gym_players=len({r["player"] for r in gym}),
                  deaths={room: sum(r["died"] for r in kept if r["room"] == room) for room in ROOMS},
                  seconds=round(time.time() - t0, 1))
    COUNTS.write_text(json.dumps(counts, indent=1))
    print(json.dumps({k: v for k, v in counts.items()
                      if k not in ("drop_reasons", "potions_left_out")}, indent=1))
    print("drop reasons:", dict(why.most_common(15)))
    print("potions left out:", dict(potions.most_common(10)))
    table(gym, f"gym ({GYM})")
    table(panel, f"panel ({PANEL}, {len(panel_players)} players)")
    return 0


# ---------------------------------------------------------------------------------------------
# Bench


def read_jsonl(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def play_gym(net, dev, panel, seed=BENCH_SEED):
    """Greedy outcome of every panel fight on seed `seed + k`: [{k, win, died, hp_left, end}]."""
    import rln36_boss as B
    from sts2 import HOLD
    from sts2.env import RUN_ENDS

    tmp = DATA / f"bench-{os.getpid()}.jsonl"
    tmp.write_text("".join(engine_line(r) + "\n" for r in panel))
    n = len(panel)
    env = B.make_env(net, n, seed_range=(0, 1), cycle=True)
    try:
        env.load_gym(tmp)
        env.set_gym(first=0, slots=n, sequential=True, seed=seed)
        env.reset_all()
        assert (env.cell >= 7).all(), "every env plays its panel fight"
        live = np.ones(n, dtype=bool)
        res = [None] * n
        for _ in range(STEP_CAP):
            idx = np.nonzero(live)[0]
            if len(idx) == 0:
                break
            a = np.concatenate([B.logits_of(net, env, idx[k:k + CHUNK], dev).argmax(-1).cpu().numpy()
                                for k in range(0, len(idx), CHUNK)])
            acts = np.full(n, HOLD, dtype=np.uint16)
            acts[idx] = a
            env.step(acts)
            for i in idx[env.done[idx].astype(bool)]:
                live[i] = False
                end = RUN_ENDS[env.end[i]]
                hp_left = hp_of(env.final_obs[i]) if end in ("gym_end", "victory") else 0
                res[i] = dict(k=int(i), end=end, died=int(end not in ("gym_end", "victory")),
                              hp_left=int(hp_left))
        for i in np.nonzero(live)[0]:
            res[i] = dict(k=int(i), end="capped", died=1, hp_left=0)
    finally:
        env.close()
        tmp.unlink(missing_ok=True)
    return res


def hp_of(obs):
    """The run header's HP (`run_obs.rs`, bytes 8-9): after the fight, at the first decision out."""
    return int(np.int16(int(obs[8]) | int(obs[9]) << 8))


def cell_rows(panel, res):
    """Per (act, room): n, deaths/100, HP lost % for the net and the humans."""
    out = []
    keys = sorted({(r["act"], r["room"]) for r in panel}, key=lambda k: (ROOMS.index(k[1]), k[0]))
    for key in keys + [("all", "elite"), ("all", "boss")]:
        sel = [i for i, r in enumerate(panel)
               if r["room"] == key[1] and (key[0] == "all" or r["act"] == key[0])]
        if not sel:
            continue
        lost = [lost_pct(panel[i], res[i]) for i in sel]
        out.append(dict(act=key[0] if key[0] == "all" else key[0] + 1, room=key[1], n=len(sel),
                        deaths=100 * np.mean([res[i]["died"] for i in sel]),
                        hp_lost=float(np.mean(lost)),
                        human_deaths=100 * np.mean([panel[i]["died"] for i in sel]),
                        human_hp_lost=float(np.mean([100 * panel[i]["hp_lost"] / panel[i]["max_hp"]
                                                     for i in sel]))))
    return out


def lost_pct(r, o):
    """HP lost in % of max HP; a death loses its entry HP."""
    if o["died"]:
        return 100 * r["hp"] / r["max_hp"]
    return 100 * max(0, r["hp"] - o["hp_left"]) / r["max_hp"]


def show_cells(rows):
    print("| act | room | n | deaths/100 | HP lost % | human deaths/100 | human HP lost % |\n"
          "|---|---|---:|---:|---:|---:|---:|")
    for r in rows:
        print(f"| {r['act']} | {r['room']} | {r['n']} | {r['deaths']:.1f} | {r['hp_lost']:.1f} "
              f"| {r['human_deaths']:.1f} | {r['human_hp_lost']:.1f} |")


def cmd_bench(args):
    import rln36_boss as B
    import rln39_fightbench as FB
    import rln19_act2 as R

    t0 = time.time()
    panel = read_jsonl(args.panel)
    if args.limit:
        panel = panel[:args.limit]
    dev, net = B.device_and_net(args.ckpt, args.weights)
    res = play_gym(net, dev, panel)
    rows = cell_rows(panel, res)
    show_cells(rows)
    t1 = time.time()
    print(f"{len(panel)} human-panel fights in {t1 - t0:.0f}s")
    own = None
    if args.own_panel != "none":
        _, op = R.read_jsonl(args.own_panel)
        own_res = FB.play(net, dev, op)
        own = dict(summary=FB.summarize(op, own_res), fights=own_res)
        FB.show(own["summary"])
        print(f"{len(op)} own-entry fights in {time.time() - t1:.0f}s")
    if args.out:
        import e2e_eval as EV
        pathlib.Path(args.out).write_text(json.dumps(
            {"ckpt": str(args.ckpt), "ckpt_sha256": EV.file_sha256(args.ckpt),
             "weights": args.weights, "panel": str(args.panel), "n": len(panel),
             "seed": BENCH_SEED, "seconds": time.time() - t0, "cells": rows, "fights": res,
             "own": own}))
    return 0


# ---------------------------------------------------------------------------------------------
# Compare


def paired_cells(panel, a, b):
    """Per cell: A - B deaths/100 (Newcombe paired CI) and HP lost % (paired t CI)."""
    import e2e_eval as EV

    out = []
    keys = sorted({(r["act"], r["room"]) for r in panel}, key=lambda k: (ROOMS.index(k[1]), k[0]))
    for key in keys + [("all", "elite"), ("all", "boss")]:
        sel = [i for i, r in enumerate(panel)
               if r["room"] == key[1] and (key[0] == "all" or r["act"] == key[0])]
        if not sel:
            continue
        da = [a[i]["died"] for i in sel]
        db = [b[i]["died"] for i in sel]
        both = sum(x and y for x, y in zip(da, db))
        only_a = sum(x and not y for x, y in zip(da, db))
        only_b = sum(y and not x for x, y in zip(da, db))
        neither = len(sel) - both - only_a - only_b
        d, lo, hi = EV.newcombe_paired(both, only_a, only_b, neither)
        diff = np.array([lost_pct(panel[i], a[i]) - lost_pct(panel[i], b[i]) for i in sel])
        se = diff.std(ddof=1) / math.sqrt(len(diff)) if len(diff) > 1 else float("nan")
        out.append(dict(act=key[0] if key[0] == "all" else key[0] + 1, room=key[1], n=len(sel),
                        a=100 * np.mean(da), b=100 * np.mean(db), d=100 * d, lo=100 * lo,
                        hi=100 * hi, hp=float(diff.mean()), hp_lo=float(diff.mean() - 1.96 * se),
                        hp_hi=float(diff.mean() + 1.96 * se)))
    return out


def cmd_compare(args):
    runs = [json.loads(pathlib.Path(p).read_text()) for p in args.results]
    panel = read_jsonl(runs[0]["panel"])[:runs[0]["n"]]
    names = [pathlib.Path(p).stem for p in args.results]
    base = runs[-1]
    for name, r in zip(names[:-1], runs[:-1]):
        print(f"\n{name} minus {names[-1]} (deaths/100, Newcombe paired 95%; HP lost %, paired t)")
        print("| act | room | n | A deaths | B deaths | diff | 95% | HP lost diff | 95% |\n"
              "|---|---|---:|---:|---:|---:|---|---:|---|")
        for c in paired_cells(panel, r["fights"], base["fights"]):
            print(f"| {c['act']} | {c['room']} | {c['n']} | {c['a']:.1f} | {c['b']:.1f} "
                  f"| {c['d']:+.1f} | {c['lo']:+.1f} to {c['hi']:+.1f} | {c['hp']:+.1f} "
                  f"| {c['hp_lo']:+.1f} to {c['hp_hi']:+.1f} |")
    return 0


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    sub.add_parser("build")
    b = sub.add_parser("bench")
    b.add_argument("ckpt")
    b.add_argument("--weights", default="ema0.999")
    b.add_argument("--panel", default=str(PANEL))
    b.add_argument("--own-panel", default=str(pathlib.Path(__file__).resolve().parent
                                              / "rln39/panel-v1.jsonl.gz"))
    b.add_argument("--limit", type=int, default=0)
    b.add_argument("--out")
    c = sub.add_parser("compare")
    c.add_argument("results", nargs="+", help="bench outputs; the last is B")
    args = p.parse_args()
    return {"build": cmd_build, "bench": cmd_bench, "compare": cmd_compare}[args.cmd](args)


if __name__ == "__main__":
    raise SystemExit(main())
