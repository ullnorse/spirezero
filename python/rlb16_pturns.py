"""Boss fights: potions held, used, and used on turns 1 and 2, by boss and outcome.
usage: rlb16_pturns.py LOG.jsonl... (classes.tsv from rlb16_classes.py)"""
import json, sys, collections, statistics as st
cls = dict(l.split() for l in open("classes.tsv"))
runs = collections.defaultdict(list)
for f in sys.argv[1:]:
    for line in open(f):
        r = json.loads(line); runs[r["seed"]].append(r)
g = collections.defaultdict(list)
for seed, L in runs.items():
    end = L[-1]["end"]
    for l in L:
        if l["t"] != "fight" or l["kind"] != "boss": continue
        fatal = end != "victory" and l is [x for x in L if x["t"] == "fight"][-1]
        boss = l["room"].split(":")[1]
        key = (l["act"], boss if l["act"] == 3 else "act%d" % l["act"], "lost-" + cls[seed] if fatal else "won")
        pt = l["potion_turns"]; n = len(l["potions"])
        g[key].append((n, len(pt), sum(t <= 1 for t in pt), sum(t <= 2 for t in pt)))
print("%-28s %5s %7s %6s %6s %6s %8s" % ("boss/outcome", "n", "potions", "used", "T1", "T<=2", "T1/used"))
for k in sorted(g):
    v = g[k]; used = sum(x[1] for x in v)
    print("%-28s %5d %7.2f %6.2f %6.2f %6.2f %8.2f" % ("/".join(map(str, k[1:])), len(v), st.mean(x[0] for x in v), st.mean(x[1] for x in v), st.mean(x[2] for x in v), st.mean(x[3] for x in v), sum(x[2] for x in v) / max(used, 1)))
