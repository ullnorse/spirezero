"""Paired fight screen (VN1): two `sts2-diff fights` outputs on the same starts (rows: seed,
kind, encounter, won, HP at the start, HP left, ms). Wins, discordant pairs with a two-sided
sign test, HP left (mean difference ± standard error), by encounter.

    python3 python/vn1_paired.py BASE VARIANT
"""

import math
import sys
from collections import defaultdict


def rows(path):
    out = {}
    for line in open(path):
        f = line.split("\t")
        if len(f) >= 7:
            out[(f[0], f[2], f[4])] = (int(f[3]), int(f[5]), int(f[6]))
    return out


def sign_p(a, b):
    n, k = a + b, min(a, b)
    if n == 0:
        return 1.0
    return min(1.0, 2 * sum(math.comb(n, i) for i in range(k + 1)) / 2 ** n)


def summary(tag, pairs):
    n = len(pairs)
    if n == 0:
        return
    bw = sum(b[0] for b, _ in pairs)
    vw = sum(v[0] for _, v in pairs)
    lost_b = sum(1 for b, v in pairs if b[0] and not v[0])
    lost_v = sum(1 for b, v in pairs if v[0] and not b[0])
    d = [v[1] - b[1] for b, v in pairs]
    m = sum(d) / n
    se = math.sqrt(sum((x - m) ** 2 for x in d) / max(n - 1, 1) / n)
    ms = sum(v[2] for _, v in pairs) / max(sum(b[2] for b, _ in pairs), 1)
    print(f"{tag:<22} n {n:4d} base {bw:4d} variant {vw:4d} discordant {lost_b}/{lost_v} "
          f"(p {sign_p(lost_b, lost_v):.2f}) HP left {m:+.2f}±{se:.2f} time ×{ms:.2f}")


def main():
    base, var = rows(sys.argv[1]), rows(sys.argv[2])
    keys = sorted(set(base) & set(var))
    pairs = [(base[k], var[k]) for k in keys]
    summary("all", pairs)
    by = defaultdict(list)
    for k in keys:
        by[k[1]].append((base[k], var[k]))
    for e in sorted(by, key=lambda e: -len(by[e])):
        summary(f"  {e}", by[e])


if __name__ == "__main__":
    main()
