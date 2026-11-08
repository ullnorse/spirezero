"""Paired whole-run A/B against a pooled base (RLB18).
usage: pooled.py VAR/progress.tsv BASE1.tsv [BASE2.tsv ...] [--smooth VAR_SMOOTH.tsv BASE_SMOOTH.tsv]
Plain: per seed, the variant's win (0/1) against the mean of the base draws that have the seed;
each draw's McNemar line too. Smoothed: per seed, the share of reseeded act-3 boss copies won
(k = 0..8, fights rows), 0 for a run that died before the act-3 boss; paired t over seeds."""
import sys, math
def rows(p):
    out = {}
    for line in open(p):
        x = line.rstrip("\n").split("\t")
        if len(x) > 1 and x[0].startswith("RUN") and not x[1].startswith("error"):
            out[int(x[0][3:])] = x[1] == "victory"
    return out
def mcnemar(bo, vo):
    d = bo + vo; k = min(bo, vo)
    return min(1.0, 2 * sum(math.comb(d, i) for i in range(k + 1)) / 2 ** d) if d else 1.0
def ttest(ds):
    n = len(ds); m = sum(ds) / n; v = sum((d - m) ** 2 for d in ds) / max(n - 1, 1)
    se = math.sqrt(v / n) if v else 0; z = m / se if se else 0
    return m, 2 * (1 - 0.5 * (1 + math.erf(abs(z) / math.sqrt(2))))
a = sys.argv[1:]; smooth = None
if "--smooth" in a:
    i = a.index("--smooth"); smooth = a[i + 1:i + 3]; a = a[:i]
var = rows(a[0]); bases = [rows(b) for b in a[1:]]
seeds = sorted(s for s in var if any(s in b for b in bases))
# the variant's finished prefix
s0 = min(var); seeds = [s for s in seeds if s >= s0]
for name, b in zip(a[1:], bases):
    ks = [s for s in seeds if s in b]
    bo = sum(b[s] and not var[s] for s in ks); vo = sum(var[s] and not b[s] for s in ks)
    print(f"  vs {name}: n {len(ks)}  base {sum(b[s] for s in ks)}  variant {sum(var[s] for s in ks)}  discordant {bo}/{vo}  p {mcnemar(bo, vo):.3f}")
ds = [var[s] - sum(b[s] for b in bases if s in b) / sum(s in b for b in bases) for s in seeds]
m, p = ttest(ds)
pb = sum(sum(b[s] for b in bases if s in b) / sum(s in b for b in bases) for s in seeds)
print(f"pooled: n {len(seeds)}  base {pb:.1f}  variant {sum(var[s] for s in seeds)}  diff {1000*m:+.1f}/1000  p {p:.3f}")
if smooth:
    def sm(p, alive):
        w = {}; n = {}
        for l in open(p):
            x = l.rstrip("\n").split("\t")
            if len(x) >= 8 and x[1] == "boss":
                s = int(x[0]); w[s] = w.get(s, 0) + int(x[3]); n[s] = n.get(s, 0) + 1
        return {s: w[s] / n[s] for s in w}
    sv, sb = sm(smooth[0], var), sm(smooth[1], bases[0])
    b0 = bases[0]; ks = [s for s in seeds if s in b0]
    # a run that died before its act-3 boss scores 0; one that reached it, its copies' share
    ds = [sv.get(s, 0) - sb.get(s, 0) for s in ks]
    m, p = ttest(ds)
    print(f"smoothed vs {a[1]}: n {len(ks)}  base {sum(sb.get(s,0) for s in ks):.1f}  variant {sum(sv.get(s,0) for s in ks):.1f}  diff {1000*m:+.1f}/1000  p {p:.3f}")
