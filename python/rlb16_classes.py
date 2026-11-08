"""RLB16 loss classes: search-losable (some ceiling variant wins the fatal fight) or pre-decided.
usage: rlb16_classes.py GAP_TSV... ; reads feats.tsv"""
import csv, sys, collections, statistics as st
win = collections.defaultdict(bool); seen = set()
for f in sys.argv[1:]:
    for line in open(f):
        x = line.split("\t"); seen.add("RUN" + x[0]); win["RUN" + x[0]] |= x[3] == "1"
rows = list(csv.DictReader(open("feats.tsv"), delimiter="\t"))
lost = [r for r in rows if r["fatal"] == "1"]
cls = {r["seed"]: ("a" if win[r["seed"]] else "b") for r in lost}
print("losses", len(lost), "replayed", len(seen), "a", sum(v == "a" for v in cls.values()), "b", sum(v == "b" for v in cls.values()))
by = collections.defaultdict(lambda: [0, 0])
for r in lost: by[r["room"]][cls[r["seed"]] == "b"] += 1
print("per fight (a/b):", {k: f"{v[0]}/{v[1]}" for k, v in sorted(by.items(), key=lambda kv: -sum(kv[1]))})
keys = ["hp","max_hp","hp_pct","gold","deck","strikes","defends","curses","upgrades","relics","potions",
        "removals","elites","rests_heal","rests_other","shops","spent","picked","skipped","hp_lost_fights"]
for scope, sel in (("act-3 boss", lambda r: r["act"] == "3" and r["kind"] == "boss"),
                   ("other bosses", lambda r: r["act"] != "3" and r["kind"] == "boss")):
    g = {"won": [r for r in rows if r["won"] == "1" and sel(r)],
         "a": [r for r in lost if sel(r) and cls[r["seed"]] == "a"],
         "b": [r for r in lost if sel(r) and cls[r["seed"]] == "b"]}
    print(f"\n== {scope}")
    print("%-14s" % "" + "".join("%9s" % k for k in g))
    for k in keys:
        print("%-14s" % k + "".join("%9.1f" % st.mean(float(r[k]) for r in v) if v else "%9s" % "-" for v in g.values()))
    print("%-14s" % "n" + "".join("%9d" % len(v) for v in g.values()))
with open("classes.tsv", "w") as f:
    for s, c in sorted(cls.items()): f.write(f"{s}\t{c}\n")
