"""RLN39 §4: run-level override harvests against the base harvest, paired on seed.

    python python/rln39_overrides.py BASE.gz VARIANT.gz...

Per harvest: runs, mean floor, boss fights reached and won per act, and the paired floor
difference to BASE (mean and 95% CI by the normal approximation of the per-seed differences).
"""

import math
import pathlib
import sys

import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import rln19_act2 as R  # noqa: E402


def summary(path):
    meta, fights = R.read_jsonl(path)
    floors = {r["seed"]: r["floor"] for r in meta["runs"]}
    acts = {a: [f for f in fights if f["act"] == a] for a in (1, 2, 3)}
    return meta, floors, {a: (len(fs), sum(f["end"] == "won" for f in fs)) for a, fs in acts.items()}


def main(base, *variants):
    rows = []
    mb, fb, ab = summary(base)
    for p in (base, *variants):
        meta, fl, ac = summary(p)
        name = pathlib.Path(p).name.removesuffix(".gz")
        seeds = sorted(set(fl) & set(fb))
        d = np.array([fl[s] - fb[s] for s in seeds], dtype=float)
        se = d.std(ddof=1) / math.sqrt(len(d)) if len(d) > 1 else float("nan")
        rows.append(f"| {name} | {len(fl)} | {dict(meta.get('overrides') or {})} | {np.mean(list(fl.values())):.2f} "
                    f"| {ac[1][0]} / {ac[1][1]} ({100 * ac[1][1] / max(1, ac[1][0]):.0f}%) "
                    f"| {ac[2][0]} / {ac[2][1]} ({100 * ac[2][1] / max(1, ac[2][0]):.0f}%) "
                    f"| {ac[3][0]} / {ac[3][1]} | {d.mean():+.2f} ({d.mean() - 1.96 * se:+.2f} to {d.mean() + 1.96 * se:+.2f}) |")
    print("| harvest | runs | overrides | mean floor | act-1 boss fights / won | act-2 boss fights / won "
          "| act-3 boss fights / won | floor vs base, paired (95%) |\n|---|---:|---|---:|---|---|---|---|")
    print("\n".join(rows))


if __name__ == "__main__":
    main(*sys.argv[1:])
