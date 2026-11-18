"""CB1 diagnostic: the net's boss losses that were winnable, as readable turn-by-turn traces.

    python tools/cb1/traces.py CKPT --acts 0,1 --per-act 10 --n 16 --out F.txt

Each boss start of the benchmark (acts `acts`) is set up as in `winnable.py` (sequential gym, seed
7,000,000 + k over the boss-only list). At its first decision the fight is copied `n + 1` times
exactly (the same draws and streams): copy 0 is played greedily (the net's real play), copies
1..n sampling at T = 1. A start where the greedy copy died and a sampled copy won is a winnable
loss; the first `per-act` per act are written: per turn, HP, block, energy, the enemies' visible
incoming damage, Osty, the enemies' HP, the hand and the moves played (with the net's probability),
for the greedy loss and the first winning copy.
"""

import argparse
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
    p.add_argument("--weights", default="raw")
    p.add_argument("--acts", default="0,1")
    p.add_argument("--per-act", type=int, default=10)
    p.add_argument("--n", type=int, default=16)
    p.add_argument("--threads", type=int, default=6)
    p.add_argument("--out", required=True)
    a = p.parse_args()
    import torch

    import naz3_combat as N
    import rln19_act2 as R
    import rln36_boss as B
    from sts2 import HOLD
    from sts2.env import RUN_ENDS

    net, dev = N.net_and_dev(a.ckpt, a.weights)
    names = R._vocab("cards")

    def hand(row):
        out = []
        for slot in range(int(row[R.COMBAT_AT + 16])):  # the header's hand size
            cid = R.u16(row, R.COMBAT_AT + R.C_HAND + 2 * slot)
            at = R.COMBAT_AT + R.C_CARDS + cid * R.CARD_BYTES
            kind = R.u16(row, at)
            if not kind:
                break
            out.append((names[kind] if kind < len(names) else f"#{kind}",
                        int(np.int8(row[at + 4]))))
        return out
    allb = [r for r in N.read_jsonl(N.DATA / "bench.jsonl") if N.room_of(r) == "boss"]
    acts = {int(x) for x in a.acts.split(",")}
    keep = [k for k, r in enumerate(allb) if r["act"] in acts]
    recs = allb[: max(keep) + 1]  # the same seeds as winnable.py: k over the boss list
    F, n = len(recs), a.n + 1
    tmp = ROOT / f"target/cb1/tr-gym-{os.getpid()}.jsonl"
    tmp.write_text("".join(N.engine_line(r) + "\n" for r in recs))
    B.THREADS = a.threads
    total = F + F * n
    env = B.make_env(net, total, seed_range=(0, 1), cycle=True)
    gen = torch.Generator(device=dev)
    gen.manual_seed(1)
    trace = {}  # env -> list of (row fields, hand, move, p)
    try:
        assert env.load_gym(tmp) == F
        env.set_gym(first=0, slots=F, sequential=True, seed=7_000_000)
        env.reset_all()
        src = np.repeat(np.array(keep), n)
        dst = F + np.arange(len(src))
        env.copy(src, dst)
        greedy = np.zeros(total, dtype=bool)
        greedy[dst[::n]] = True
        start_of = dict(zip(dst.tolist(), src.tolist()))
        running, won = dst, np.zeros(total, dtype=bool)
        result = {}
        steps = 0
        while len(running) and steps < N.ROLLOUT_CAP:
            done = env.done[running].astype(bool)
            fin = done | (env.obs[running, N.H_PHASE] != N.PHASE_COMBAT)
            for i in running[fin]:
                end = RUN_ENDS[env.end[i]]
                won[i] = not env.done[i] or end in ("gym_end", "victory")
                last = env.final_obs[i] if env.done[i] else env.obs[i]
                result[int(i)] = (won[i], R.combat_row(last))
            running = running[~fin]
            if not len(running):
                break
            lg = N.logits(net, env, running, dev)
            pr = torch.softmax(lg, -1)
            samp = torch.multinomial(pr, 1, generator=gen).squeeze(1)
            act = torch.where(torch.from_numpy(greedy[running]).to(lg.device), lg.argmax(-1), samp)
            act = act.cpu().numpy()
            pa = pr[torch.arange(len(running)), torch.from_numpy(act).to(pr.device)].cpu().numpy()
            pmax = pr.max(-1).values.cpu().numpy()
            for j, i in enumerate(running):
                row = env.obs[i]
                trace.setdefault(int(i), []).append(
                    (R.combat_row(row), hand(row), B.label(row, int(act[j])),
                     float(pa[j]), float(pmax[j])))
            acts_ = np.full(total, HOLD, dtype=np.uint16)
            acts_[running] = act
            env.step(acts_)
            steps += 1
    finally:
        env.close()
        tmp.unlink(missing_ok=True)

    def show(i):
        lines, turn, shown = [], None, False
        for row, hnd, (kind, what), pa, pmax in trace[i]:
            f = dict(zip(R.ROW_FIELDS, row))
            if f["turn"] != turn:
                turn, shown = f["turn"], False
                lines.append(f"  T{turn}: hp {f['hp']} block {f['block']} E{f['energy']} incoming "
                             f"{f['incoming']} osty {f['osty_hp'] if f['osty'] else '-'} enemies "
                             f"{f['ehp']}/{f['emax']} belt {f['belt']}")
                lines.append("     plays:")
            if kind != "choice_option" and not shown:
                shown = True
                lines.insert(len(lines) - 1, "     hand: " + ", ".join(f"{c}({e})" for c, e in hnd))
            lines[-1] += f" {kind}:{what}" + (f"[p{pa:.2f}]" if pa < 0.9 else "")
            if kind == "end":
                lines[-1] += f" (block {f['block']} vs incoming {f['incoming']}, E{f['energy']} left)"
        w, last = result.get(i, (False, None))
        lines.append(f"  -> {'WON' if w else 'LOST'}; last state {last}")
        return "\n".join(lines)

    out, count = [], {k: 0 for k in acts}
    starts = 0
    for g in dst[::n]:
        k = start_of[int(g)]
        r = recs[k]
        copies = list(range(int(g), int(g) + n))
        wins = [c for c in copies[1:] if won[c]]
        starts += 1
        if won[g] or not wins or count[r["act"]] >= a.per_act:
            continue
        count[r["act"]] += 1
        out.append(f"=== act {r['act'] + 1} {r['encounter']} start {r['id']}: hp {r['hp']}/"
                   f"{r['max_hp']}, deck {len(r['deck'])}: "
                   + ", ".join(c + "+" * u for c, u in r["deck"])
                   + f"; relics {', '.join(r['relics'])}; potions {', '.join(r['potions'])}; "
                   f"sampled wins {len(wins)}/{n - 1}")
        out.append("GREEDY (lost):\n" + show(int(g)))
        out.append("A WIN (sampled):\n" + show(wins[0]))
    pathlib.Path(a.out).write_text("\n".join(out) + "\n")
    lost = sum(1 for g in dst[::n] if not won[g])
    print(f"{starts} starts, greedy lost {lost}; written {count} winnable losses to {a.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
