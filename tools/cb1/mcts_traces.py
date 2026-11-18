"""CB1 round 2 (b): turn-by-turn traces of MCTS losses on act-3 panel bosses that the human won.

    python tools/cb1/mcts_traces.py BASE_CKPT SCREEN.npz --n 10 --reps 3 --out F.txt [search args]

Takes the act-3 boss starts that the MCTS screen (`cb1_mcts.py run ... --out SCREEN.npz`) lost and
the human won, spread over the encounters, and plays each `reps` times with the same search
settings (`cb1_mcts.py` flags after the known ones). Writes the first lost copy of each start: per
turn HP, block, energy, visible incoming damage, Osty, enemy HP, the hand, and the moves played
(`*` where the search deviated from the net's greedy move, with the root's visit count).
"""

import argparse
import collections
import json
import os
import pathlib
import sys

import numpy as np

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "python"))


def main():
    p = argparse.ArgumentParser()
    p.add_argument("ckpt")
    p.add_argument("screen")
    p.add_argument("--n", type=int, default=10)
    p.add_argument("--reps", type=int, default=3)
    p.add_argument("--panel", default="~/.cache/spirebot-data/cb1/panel_bench.jsonl")
    p.add_argument("--out", required=True)
    a, rest = p.parse_known_args()
    import cb1_mcts as M
    import naz3_combat as C
    import rln19_act2 as R
    import rln36_boss as B

    sa = argparse.ArgumentParser()
    M.search_flags(sa)
    s = sa.parse_args(rest)
    net, dev = C.net_and_dev(a.ckpt, s.weights)
    recs = {r["id"]: r for r in C.read_jsonl(pathlib.Path(a.panel).expanduser())}
    z = np.load(a.screen)
    lost = [(i, e) for i, act, e, d in zip(z["ids"].tolist(), z["act"].tolist(), z["enc"].tolist(),
                                           z["died"].astype(int).tolist())
            if act == 2 and d == 1 and not recs[i]["died"]]
    by = collections.defaultdict(list)
    for i, e in lost:
        by[e].append(i)
    pick = []
    while len(pick) < a.n and any(by.values()):  # round-robin over the encounters
        for e in sorted(by):
            if by[e] and len(pick) < a.n:
                pick.append(by[e].pop(0))
    sel = [recs[i] for i in pick]
    names = R._vocab("cards")

    def hand(row):
        out = []
        for slot in range(int(row[R.COMBAT_AT + 16])):
            cid = R.u16(row, R.COMBAT_AT + R.C_HAND + 2 * slot)
            at = R.COMBAT_AT + R.C_CARDS + cid * R.CARD_BYTES
            kind = R.u16(row, at)
            if not kind:
                break
            out.append(f"{names[kind] if kind < len(names) else kind}({int(np.int8(row[at + 4]))})")
        return out

    player = M.player_of(s, not s.no_gumbel)
    trace = collections.defaultdict(list)

    def on_step(env, idx, k_of, lg, act):
        greedy = lg.argmax(-1).cpu().numpy()
        for j, (i, k) in enumerate(zip(idx, k_of)):
            row = env.obs[i]
            trace[int(k)].append((R.combat_row(row), hand(row), B.label(row, int(act[j])),
                                  int(act[j]) != int(greedy[j]), int(player.last_rows[j])))

    res = M.play(net, dev, sel, a.reps, 7_000_000, len(sel) * a.reps, player, s.threads,
                 on_step=on_step)
    out = []
    for n, r in enumerate(sel):
        ks = [n * a.reps + j for j in range(a.reps)]
        dead = [k for k in ks if res["died"][k]]
        out.append(f"=== {r['encounter']} {r['id']}: hp {r['hp']}/{r['max_hp']}, deck {len(r['deck'])}: "
                   + ", ".join(c + "+" * u for c, u in r["deck"])
                   + f"; relics {', '.join(r['relics'])}; potions {', '.join(r['potions'])}; "
                   f"human: won, lost {r['hp_lost']} HP in {r['turns']} turns; MCTS lost "
                   f"{len(dead)}/{a.reps} replays")
        if not dead:
            continue
        turn = None
        for row, hnd, (kind, what), dev_, nvis in trace[dead[0]]:
            f = dict(zip(R.ROW_FIELDS, row))
            if f["turn"] != turn:
                turn = f["turn"]
                out.append(f"  T{turn}: hp {f['hp']} block {f['block']} E{f['energy']} incoming "
                           f"{f['incoming']} osty {f['osty_hp'] if f['osty'] else '-'} enemies "
                           f"{f['ehp']}/{f['emax']}")
                out.append("     hand: " + ", ".join(hnd))
                out.append("     plays:")
            out[-1] += f" {kind}:{what}" + (f"*[{nvis}]" if dev_ else "")
        out.append("  -> LOST")
    pathlib.Path(a.out).write_text("\n".join(out) + "\n")
    print(f"{len(sel)} starts, MCTS lost {int(res['died'].sum())}/{len(res['died'])} replays; "
          f"wrote {a.out}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
