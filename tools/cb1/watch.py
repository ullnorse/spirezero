"""Prints a training run's gym win rates (rolling over the last N iterations)."""
import json, sys
path, n = sys.argv[1], int(sys.argv[2]) if len(sys.argv) > 2 else 20
L = [json.loads(l) for l in open(path)]
def agg(rows):
    out = {}
    for s in rows:
        for c, (e, w) in s.get("practice", {}).get("ended", {}).items():
            a = out.setdefault(c, [0, 0]); a[0] += e; a[1] += w
    return {c.replace("GYM_", ""): round(1 - w / max(e, 1), 3) for c, (e, w) in sorted(out.items())}
for k in range(n, len(L) + 1, max(n, len(L) // 8 // n * n)):
    s = L[k - 1]
    print(f"{s['decisions']/1e6:7.1f}M dps {s['total_dps']:.0f} ent {s['ent_combat']:.3f} ev {s['ev_combat']:.2f} deaths {agg(L[k - n:k])}")
