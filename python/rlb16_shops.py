"""Shop visits with a removable curse in the deck and the removal affordable: was it bought?
usage: rlb16_shops.py LOG.jsonl... (run-log output; classes.tsv from rlb16_classes.py)"""
import collections, json, sys
cls = dict(l.split() for l in open("classes.tsv"))
runs = collections.defaultdict(list)
for f in sys.argv[1:]:
    for line in open(f):
        r = json.loads(line); runs[r["seed"]].append(r)
stat = collections.defaultdict(collections.Counter)
for seed, L in runs.items():
    g = "won" if L[-1]["end"] == "victory" else "lost-" + cls.get(seed, "?")
    visits = collections.defaultdict(list)
    for c in L:
        if c["t"] == "choice" and c["room"] == "Shop": visits[c["floor"]].append(c)
    for cs in visits.values():
        f = cs[0]
        able = f["removable_curses"] > 0 and f["removal"] is not None and f["gold_before"] >= f["removal"]
        st = stat[g]; st["visits"] += 1; st["curse, removal affordable"] += able
        st["... removed"] += able and any(c["a"] == "RemoveCard" for c in cs)
for g, st in stat.items(): print(g, dict(st))
