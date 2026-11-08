"""Paired whole-run comparison (RLB11): `BASE` and `VARIANT` rows (`rows.tsv` or a growing
`progress.tsv` of `sts2-diff bot`) over the longest prefix of seeds both have finished; prints
n, the wins of each, the discordant pairs and a two-sided sign test on them.

    python3 python/rlb11_paired.py BASE/rows.tsv VARIANT/progress.tsv
"""

import math
import sys


def rows(path):
    out = {}
    for line in open(path):
        parts = line.rstrip("\n").split("\t")
        if len(parts) > 1 and parts[0].startswith("RUN") and not parts[1].startswith("error"):
            out[int(parts[0][3:])] = parts[1] == "victory"
    return out


def main():
    base, var = rows(sys.argv[1]), rows(sys.argv[2])
    seed = min(base)
    n = bw = vw = b_only = v_only = 0
    while seed in base and seed in var:
        n += 1
        bw += base[seed]
        vw += var[seed]
        b_only += base[seed] and not var[seed]
        v_only += var[seed] and not base[seed]
        seed += 1
    d = b_only + v_only
    k = min(b_only, v_only)
    p = min(1.0, 2 * sum(math.comb(d, i) for i in range(k + 1)) / 2 ** d) if d else 1.0
    print(f"n {n}  base {bw}  variant {vw}  discordant base/variant {b_only}/{v_only}  p {p:.3f}")


if __name__ == "__main__":
    main()
