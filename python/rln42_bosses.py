"""RLN42: floor, wins and boss wins per act of an `e2e_eval.py run` file (DEV-1000).

    python python/rln42_bosses.py RUNS.jsonl [BASE.jsonl]   # with BASE: the paired floor difference

Boss wins of entries, per act: runs that entered that act's boss fight (`fights`, `fight_acts`) and
killed it (`boss_acts` bit act - 1), over the runs that entered it.
"""

import sys

import e2e_eval as V


def bosses(runs):
    out = {}
    for a in (1, 2, 3):
        ent = [r for r in runs.values()
               if any(e.endswith("_BOSS") and fa == a for e, fa in zip(r["fights"], r["fight_acts"]))]
        won = sum(bool(r["boss_acts"] >> (a - 1) & 1) for r in ent)
        out[f"act{a}"] = (won / len(ent) if ent else None, len(ent))
    return out


def main():
    _, runs = V.load_results(sys.argv[1])
    s = V.stats_of(runs)
    print(f"runs {s['n']} floor {s['floor']:.2f} wins {s['wins']} act2-boss reach {100 * s['a2_rate']:.1f}%")
    print("boss wins of entries", {k: (round(100 * v, 1) if v is not None else None, n)
                                   for k, (v, n) in bosses(runs).items()})
    if len(sys.argv) > 2:
        _, base = V.load_results(sys.argv[2])
        seeds = sorted(set(runs) & set(base))
        m, lo, hi = V.paired_mean([runs[k]["floor"] for k in seeds], [base[k]["floor"] for k in seeds])
        print(f"paired floor vs base ({len(seeds)} seeds): {m:+.2f} (CI {lo:+.2f} to {hi:+.2f})")


if __name__ == "__main__":
    main()
