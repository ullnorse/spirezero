"""AUC of the deck probe (RLB18) at boss entries: runs that won that boss fight against runs
that lost it (all, and RLB16's classes a and b), pooled and within the same boss.
usage: rlb18_auc.py PROBE.tsv CLASSES.tsv PROGRESS.tsv [PROGRESS.tsv ...] [--act N]"""
import sys, collections
a = sys.argv[1:]; act = "3"
if "--act" in a:
    i = a.index("--act"); act = a[i + 1]; a = a[:i] + a[i + 2:]
rows = [l.rstrip("\n").split("\t") for l in open(a[0])][1:]
cls = dict(l.split() for l in open(a[1]))
killer = {}
for p in a[2:]:
    for l in open(p):
        x = l.rstrip("\n").split("\t")
        if len(x) > 8 and x[1] == "death": killer[x[0]] = (x[2].split()[1], x[8])
lost = lambda r: killer.get(r[0]) == (r[1], r[2])
def auc(pos, neg):
    if not pos or not neg: return float("nan")
    return sum(1 if p > n else 0.5 if p == n else 0 for p in pos for n in neg) / len(pos) / len(neg)
P = lambda r: int(r[3]) / max(int(r[4]), 1)
X = [r for r in rows if r[1] == act]
wins = [r for r in X if not lost(r)]
out = []
for name, f in (("all", lambda r: True), ("a", lambda r: cls.get(r[0]) == "a"), ("b", lambda r: cls.get(r[0]) == "b")):
    loss = [r for r in X if lost(r) and f(r)]
    num = den = 0
    for b in {r[2] for r in loss}:
        pw = [P(r) for r in wins if r[2] == b]; pl = [P(r) for r in loss if r[2] == b]
        if pw and pl: num += auc(pw, pl) * len(pw) * len(pl); den += len(pw) * len(pl)
    out.append(f"{name}: AUC {auc([P(r) for r in wins], [P(r) for r in loss]):.3f} (same boss {num / max(den, 1):.3f}, lost {len(loss)})")
print(f"act {act}, won {len(wins)}:  " + "  ".join(out))
hp = lambda r: int(r[6]) / max(int(r[7]), 1)
loss = [r for r in X if lost(r) and cls.get(r[0]) == "b"]
print(f"HP fraction alone, class b: AUC {auc([hp(r) for r in wins], [hp(r) for r in loss]):.3f}")
band = collections.defaultdict(lambda: [0, 0])
for r in X:
    k = min(int(P(r) * 5), 4); band[k][0] += 1; band[k][1] += lost(r)
for k in sorted(band):
    print(f"P {k / 5:.1f}-{(k + 1) / 5:.1f}: n {band[k][0]:5d} lost {band[k][1]:3d} ({100 * band[k][1] / band[k][0]:.1f}%)")
