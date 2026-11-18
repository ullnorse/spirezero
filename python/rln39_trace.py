"""RLN39 deep traces of the act-2 bosses (greedy replays of harvested entries).

    python python/rln39_trace.py trace CKPT --harvest H.gz --boss KAISER_CRAB_BOSS --out F [--per 400]
    python python/rln39_trace.py report F...
    python python/rln39_trace.py caps CKPT --eval EVAL.jsonl [--n 64]   # why capped runs end

`trace` replays each harvested prefix (sha-checked, `rln36_boss.replay`), plays the fight greedily and
records, per decision, what the net could see: turn, HP, block, energy, visible incoming damage,
the action, and the boss mechanics read from the obs bytes: for Kaiser Crab each arm's HP, alive flag,
whether it is behind the player (v2 `SIDE_AT`, its attacks deal 1.5x) and the player's facing; for The
Insatiable the Sandpit counter (power amount on the boss) and Frantic Escape plays. It also checks
that greedy reproduces the harvest outcome (`end`).

`report` prints per-boss tables: for Kaiser Crab, when the first arm dies and which, turns that end
with an arm behind the player, and arm damage dealt per turn; for The Insatiable, the Sandpit counter
at the last decision, how many losses are Sandpit kills (counter 1 at the last decision, HP above
the visible incoming), and Frantic Escapes played.
"""

import argparse
import collections
import sys
import time
import pathlib

import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import e2e_eval as EV  # noqa: E402
import rln19_act2 as R  # noqa: E402
import rln36_boss as B  # noqa: E402

V2_AT, V_SIDE = 2996, 396
COMBAT, CREATURES_AT, CB = R.COMBAT_AT, R.C_CREATURES, R.CREATURE_BYTES
POWERS_AT = 12
_VOC = {}


def voc():
    if not _VOC:
        for k in ("monsters", "powers"):
            _VOC[k] = R._vocab(k)
    return _VOC


def mechanics(row):
    """Per-creature facts of one decision: arms (Kaiser) and the Sandpit counter (Insatiable)."""
    v = voc()
    arms, sand, foe_hp = {}, 0, 0
    for k in range(R.CREATURES):
        at = COMBAT + CREATURES_AT + k * CB
        mon = R.u16(row, at + 4)
        if not mon:
            continue
        name = v["monsters"][mon]
        alive, side = int(row[at]), int(row[at + 1])
        if side == 1 and alive:
            foe_hp += R.i16(row, at + 6)
        if name in ("CRUSHER", "ROCKET"):
            arm = int(row[V2_AT + V_SIDE + 2 * k + 1])
            arms[name] = (alive, R.i16(row, at + 6), arm)
        for j in range(12):
            p = R.u16(row, at + POWERS_AT + 4 * j)
            if p and v["powers"][p] == "SANDPIT_POWER":
                sand = R.i16(row, at + POWERS_AT + 4 * j + 2)
    face = 0
    for k in range(R.CREATURES):
        at = COMBAT + CREATURES_AT + k * CB
        if R.u16(row, at + 4) == 0 and int(row[at + 1]) == 0 and row[at]:
            face = int(row[V2_AT + V_SIDE + 2 * k])
    return arms, sand, face


def trace_fights(net, dev, fights):
    from sts2 import HOLD

    n = len(fights)
    env = B.replay(net, fights, 1)
    live = np.ones(n, dtype=bool)
    out = [{"id": f"{f['seed']}-{f['floor']}", "boss": f["boss"], "harvest_end": f["end"], "rows": [],
            "end": None} for f in fights]
    try:
        for _ in range(4000):
            idx = np.nonzero(live)[0]
            if not len(idx):
                break
            lg = B.logits_of(net, env, idx, dev)
            a = lg.argmax(-1).cpu().numpy()
            for j, i in enumerate(idx):
                row = env.obs[i]
                arms, sand, face = mechanics(row)
                cr = R.combat_row(row)
                kind, what = B.label(row, int(a[j]))
                out[i]["rows"].append({"t": cr[0], "hp": cr[1], "blk": cr[2], "en": cr[3], "inc": cr[4],
                                       "ehp": cr[7], "arms": arms, "sand": sand, "face": face,
                                       "kind": kind, "what": what})
            acts = np.full(n, HOLD, dtype=np.uint16)
            acts[idx] = a
            env.step(acts)
            for i in idx:
                e = B.fight_end(env, i)
                if e is not None:
                    live[i] = False
                    out[i]["end"] = e
    finally:
        env.close()
    return out


def cmd_trace(args):
    dev, net = B.device_and_net(args.ckpt, args.weights)
    _, fights = R.read_jsonl(args.harvest)
    sel = [f for f in fights if f["boss"] == args.boss][: args.per]
    res = []
    t0 = time.time()
    for c in range(0, len(sel), args.chunk):
        res += trace_fights(net, dev, sel[c:c + args.chunk])
        print(f"{len(res)}/{len(sel)} {time.time() - t0:.0f}s", flush=True)
    same = sum(r["end"] == r["harvest_end"] for r in res)
    print(f"greedy reproduces the harvest outcome in {same}/{len(res)} fights", flush=True)
    R.write_jsonl_gz(args.out, res, {"kind": "rln39-trace", "boss": args.boss,
                                     "ckpt_sha256": EV.file_sha256(args.ckpt)})
    return 0


def turn_ends(rows):
    """The last decision row of each turn."""
    last = collections.OrderedDict()
    for r in rows:
        last[r["t"]] = r
    return list(last.values())


def kaiser_stats(f):
    """Arms leave the creature list when they die, so an arm's death is the first decision without it."""
    rows, won = f["rows"], f["end"] == "won"
    first = {}
    for r in rows:
        for name in ("CRUSHER", "ROCKET"):
            if name not in r["arms"] and name not in first:
                first[name] = r["t"]
    ends = turn_ends(rows)
    behind = sum(any(a[2] == 1 for a in r["arms"].values()) for r in ends)
    both = sum(len(r["arms"]) == 2 for r in ends)
    flips = sum(1 for p, q in zip(rows, rows[1:]) if p["face"] and q["face"] and p["face"] != q["face"])
    arm_dmg = sum(max(0, p["arms"][n][1] - q["arms"].get(n, (0, 0, 0))[1]) if n in q["arms"]
                  else p["arms"][n][1] for p, q in zip(rows, rows[1:]) for n in p["arms"])
    first_arm = min(first, key=first.get) if first else None
    return dict(won=won, turns=len(ends), first_turn=min(first.values()) if first else None,
                first_arm=first_arm, behind=behind / max(1, len(ends)), both=both / max(1, len(ends)),
                flips=flips / max(1, len(ends)), arm_dmg=arm_dmg / max(1, len(ends)),
                killed=len(first), hp_end=rows[-1]["hp"])


def insat_stats(f):
    rows, won = f["rows"], f["end"] == "won"
    last = rows[-1]
    fe = sum(r["what"] == "FRANTIC_ESCAPE" for r in rows)
    sand = [r["sand"] for r in rows if r["sand"]]
    return dict(won=won, turns=len(turn_ends(rows)), fe=fe, sand_last=last["sand"],
                sandpit_death=(not won) and last["sand"] == 1 and last["hp"] + last["blk"] > last["inc"],
                min_sand=min(sand) if sand else None)


def cmd_report(args):
    import statistics as st

    for p in args.files:
        meta, fights = R.read_jsonl(p)
        boss = meta["boss"]
        print(f"## {boss} ({len(fights)} fights)")
        if boss == "KAISER_CRAB_BOSS":
            S = [kaiser_stats(f) for f in fights]
            print("| outcome | n | turns | first arm dies (turn) | arms killed | turns ending with an arm behind "
                  "| both arms alive at turn end | facing flips/turn | arm dmg/turn |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|")
            for name, sel in (("won", [s for s in S if s["won"]]), ("lost", [s for s in S if not s["won"]])):
                if not sel:
                    continue
                ft = [s["first_turn"] for s in sel if s["first_turn"]]
                print(f"| {name} | {len(sel)} | {st.mean(s['turns'] for s in sel):.1f} "
                      f"| {st.mean(ft) if ft else float('nan'):.1f} ({len(ft)}/{len(sel)}) "
                      f"| {st.mean(s['killed'] for s in sel):.2f} | {100 * st.mean(s['behind'] for s in sel):.0f}% "
                      f"| {100 * st.mean(s['both'] for s in sel):.0f}% | {st.mean(s['flips'] for s in sel):.2f} "
                      f"| {st.mean(s['arm_dmg'] for s in sel):.1f} |")
            c = collections.Counter((s["won"], s["first_arm"]) for s in S)
            print("\nfirst arm killed (won/lost):", {("won" if k[0] else "lost", k[1]): v for k, v in sorted(c.items(), key=str)})
            lost = [s for s in S if not s["won"]]
            print(f"lost with no arm killed: {sum(s['killed'] == 0 for s in lost)}/{len(lost)}; "
                  f"one arm only: {sum(s['killed'] == 1 for s in lost)}/{len(lost)}")
        else:
            S = [insat_stats(f) for f in fights]
            lost = [s for s in S if not s["won"]]
            print("| outcome | n | turns | Frantic Escapes played |\n|---|---:|---:|---:|")
            for name, sel in (("won", [s for s in S if s["won"]]), ("lost", lost)):
                if sel:
                    print(f"| {name} | {len(sel)} | {st.mean(s['turns'] for s in sel):.1f} "
                          f"| {st.mean(s['fe'] for s in sel):.2f} |")
            sd = sum(s["sandpit_death"] for s in lost)
            print(f"\nSandpit kills (counter 1 at the last decision, HP+block above visible incoming): "
                  f"{sd}/{len(lost)} losses ({100 * sd / max(1, len(lost)):.0f}%)")
            print("counter at the last decision, losses:",
                  dict(sorted(collections.Counter(s["sand_last"] for s in lost).items())))
        print()


def cmd_caps(args):
    """Replay the capped runs of an eval file greedily: which cap ends them, in which fight, at which
    turn, with how many decisions."""
    import json

    from sts2 import HOLD
    from sts2.env import RUN_ENDS

    dev, net = B.device_and_net(args.ckpt, args.weights)
    runs = [json.loads(line) for line in open(args.eval)][1:]
    seeds = [r["seed"] for r in runs if r["end"] == "capped"][: args.n]
    n = len(seeds)
    env = B.make_env(net, n, seeds=np.asarray(seeds, dtype=np.uint64))
    names = EV.encounter_names()
    steps = np.zeros(n, dtype=np.int64)
    last = [None] * n
    tail = [collections.deque(maxlen=40) for _ in range(n)]
    where = [None] * n
    live = np.ones(n, dtype=bool)
    try:
        while live.any():
            idx = np.nonzero(live)[0]
            a = B.logits_of(net, env, idx, dev).argmax(-1).cpu().numpy()
            for j, i in enumerate(idx):
                row = env.obs[i]
                f = EV.fight_of(row)
                if f is not None and int(row[R.H_PHASE]) == R.PHASE_COMBAT:
                    where[i] = names[f[1]]
                    last[i] = (R.combat_row(row)[0], R.combat_row(row)[1], R.combat_row(row)[7],
                               B.label(row, int(a[j])))
            for j, i in enumerate(idx):
                tail[i].append((int(env.obs[i][R.H_PHASE]), int(a[j]),
                                int(env.obs[i][EV.H_MODEL]) | int(env.obs[i][EV.H_MODEL + 1]) << 8))
            acts = np.full(n, HOLD, dtype=np.uint16)
            acts[idx] = a
            env.step(acts)
            steps[idx] += 1
            for i in idx:
                if env.done[i]:
                    live[i] = False
                    ph = collections.Counter(p for p, _, _ in tail[i])
                    ac = collections.Counter(x for _, x, _ in tail[i]).most_common(3)
                    ev = R._vocab("events")
                    mid = collections.Counter(m for _, _, m in tail[i]).most_common(1)[0][0]
                    print(f"event {ev[mid] if mid < len(ev) else mid}")
                    print(f"{seeds[i]} {RUN_ENDS[env.end[i]]} floor {int(env.floor[i])} fight {where[i]} "
                          f"turn/hp/enemyHP/last action {last[i]} decisions {steps[i]} "
                          f"last-40 phases {dict(ph)} top actions {ac}", flush=True)
    finally:
        env.close()
    return 0


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    t = sub.add_parser("trace")
    t.add_argument("ckpt")
    t.add_argument("--weights", default="ema0.999")
    t.add_argument("--harvest", required=True)
    t.add_argument("--boss", required=True)
    t.add_argument("--per", type=int, default=400)
    t.add_argument("--chunk", type=int, default=200)
    t.add_argument("--out", required=True)
    c = sub.add_parser("caps")
    c.add_argument("ckpt")
    c.add_argument("--weights", default="ema0.999")
    c.add_argument("--eval", required=True)
    c.add_argument("--n", type=int, default=64)
    r = sub.add_parser("report")
    r.add_argument("files", nargs="+")
    args = p.parse_args()
    return {"trace": cmd_trace, "report": cmd_report, "caps": cmd_caps}[args.cmd](args)


if __name__ == "__main__":
    raise SystemExit(main())
