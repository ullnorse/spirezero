"""Run anatomy: features at a fight's entry, for losses (their fatal fight) and wins (same fight).
usage: rlb16_anatomy.py LOG.jsonl [LOG.jsonl ...]   -> prints tables; writes feats.tsv"""
import json, sys, collections, statistics as st
runs = collections.defaultdict(list)
for f in sys.argv[1:]:
    for line in open(f):
        r = json.loads(line); runs[r["seed"]].append(r)
CURSES = {"GREED","REGRET","DOUBT","SHAME","PARASITE","INJURY","CLUMSY","DECAY","WRITHE","NORMALITY","PAIN","ASCENDERS_BANE","GUILTY","DEBT","BAD_LUCK","POOR_SLEEP","SPORE_MIND","CURSE_OF_THE_BELL","FOLLY","ENTHRALLED","TORIC_TOUGHNESS"}
def feats(lines, upto):
    """Features at the fight line `upto` (index into lines)."""
    f = lines[upto]; deck = f["deck"]; prior = lines[:upto]
    ch = [l for l in prior if l["t"] == "choice"]; fights = [l for l in prior if l["t"] == "fight"]
    base = [c.rstrip("+") for c in deck]
    rest = [c for c in ch if c["d"] == "RestSite"]
    return dict(
        act=f["act"], kind=f["kind"], room=f["room"].split(":")[-1], floor=f["floor"],
        hp=f["hp"], max_hp=f["max_hp"], hp_pct=round(100 * f["hp"] / f["max_hp"]), gold=f["gold"],
        deck=len(deck), strikes=sum(c.startswith("STRIKE") for c in base),
        defends=sum(c.startswith("DEFEND") for c in base), curses=sum(c in CURSES for c in base),
        upgrades=sum(c.endswith("+") for c in deck), relics=len(f["relics"]), potions=len(f["potions"]),
        removals=f["removals"], elites=sum(l["kind"] == "elite" for l in fights),
        fights=len(fights),
        rests_heal=sum(c["hp"] > 0 for c in rest), rests_other=sum(c["hp"] <= 0 for c in rest),
        shops=len({c["floor"] for c in ch if c["room"] == "Shop"}),
        spent=-sum(c["gold"] for c in ch if c["gold"] < 0),
        picked=sum(c["a"].startswith("PickCard") for c in ch),
        skipped=sum(c["a"].startswith("SkipReward") and c["d"] == "Rewards" for c in ch),
        hp_lost_fights=sum(l["hp"] - l["hp_after"] for l in fights),
        deck_list=deck, relic_list=f["relics"], potion_list=f["potions"],
    )
def table(groups, keys):
    print("%-14s" % "feature" + "".join("%12s" % g for g in groups))
    for k in keys:
        row = []
        for g, rows in groups.items():
            v = [r[k] for r in rows]; row.append("%12.1f" % st.mean(v) if v else "%12s" % "-")
        print("%-14s" % k + "".join(row))
    print("%-14s" % "n" + "".join("%12d" % len(r) for r in groups.values()))
if __name__ == "__main__":
    out = []
    for seed, lines in runs.items():
        end = lines[-1]["end"]; fl = [i for i, l in enumerate(lines) if l["t"] == "fight"]
        if end == "victory":
            for i in fl:
                if lines[i]["kind"] in ("boss", "elite"):
                    out.append(dict(seed=seed, won=1, fatal=0, **feats(lines, i)))
        else:
            out.append(dict(seed=seed, won=0, fatal=1, **feats(lines, fl[-1])))
    keys = ["floor","hp","max_hp","hp_pct","gold","deck","strikes","defends","curses","upgrades","relics",
            "potions","removals","elites","rests_heal","rests_other","shops","spent","picked","skipped","hp_lost_fights"]
    for act in (1, 2, 3):
        for kind in ("elite", "boss"):
            g = {"won": [r for r in out if r["won"] and r["act"] == act and r["kind"] == kind],
                 "lost": [r for r in out if r["fatal"] and r["act"] == act and r["kind"] == kind]}
            if g["lost"]:
                print(f"\n== act {act} {kind}"); table(g, keys)
    with open("feats.tsv", "w") as f:
        cols = ["seed","won","fatal","act","kind","room"] + keys
        f.write("\t".join(cols) + "\n")
        for r in out: f.write("\t".join(str(r[c]) for c in cols) + "\n")
