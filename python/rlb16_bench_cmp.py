"""Paired benchmark: fights rows (seed, kind, encounter, won, hp0, hp, ms, k) of a base and variants.
Reseeds k >= 1 only (k = 0 chose the set). usage: cmp.py BASE VAR..."""
import sys, math, collections
def rows(p):
    out = {}
    for l in open(p):
        x = l.rstrip("\n").split("\t")
        if len(x) >= 8 and x[7] != "0": out[(x[0], x[7])] = (x[2], int(x[3]), int(x[5]))
    return out
def ci(w, n):
    p = w / n; return 100 * p, 196 * math.sqrt(p * (1 - p) / n)
base = rows(sys.argv[1])
for f in [sys.argv[1]] + sys.argv[2:]:
    v = rows(f); keys = [k for k in v if k in base]
    for enc in sorted({base[k][0] for k in keys}) + ["all"]:
        ks = [k for k in keys if enc == "all" or base[k][0] == enc]
        w = sum(v[k][1] for k in ks); bw = sum(base[k][1] for k in ks)
        bo = sum(base[k][1] and not v[k][1] for k in ks); vo = sum(v[k][1] and not base[k][1] for k in ks)
        d = bo + vo; p = min(1.0, 2 * sum(math.comb(d, i) for i in range(min(bo, vo) + 1)) / 2 ** d) if d else 1.0
        hp = sum(v[k][2] for k in ks) / len(ks); r, e = ci(w, len(ks))
        print(f"{f.split('/')[-1]:24s} {enc:18s} n {len(ks):5d} won {w:5d} ({r:5.1f}% ±{e:3.1f})  base {bw}  disc {bo}/{vo}  p {p:.3f}  hp {hp:5.1f}")
