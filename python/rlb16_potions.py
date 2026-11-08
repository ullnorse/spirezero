"""Potions through the run (used per act and in elites, bought, at the act-3 boss) for runs
that reach the act-3 boss, wins against losses. usage: rlb16_potions.py LOG.jsonl..."""
import json, sys, collections, statistics as st
runs = collections.defaultdict(list)
for f in sys.argv[1:]:
    for line in open(f):
        r = json.loads(line); runs[r["seed"]].append(r)
agg = {"won": collections.defaultdict(list), "lost": collections.defaultdict(list)}
for seed, L in runs.items():
    end = L[-1]["end"]; fl = [l for l in L if l["t"] == "fight"]
    last = fl[-1]
    if not (last["act"] == 3 and last["kind"] == "boss"): continue
    g = agg["won" if end == "victory" else "lost"]
    for act in (1, 2, 3):
        F = [l for l in fl if l["act"] == act and l is not last]
        used = sum(max(0, len(l["potions"]) - len(l.get("potions_after", l["potions"]))) for l in F)
        g[f"used_a{act}"].append(used)
        g[f"used_elite_a{act}"].append(sum(max(0, len(l["potions"]) - len(l.get("potions_after", l["potions"]))) for l in F if l["kind"]=="elite"))
        C = [c for c in L if c["t"] == "choice" and c["act"] == act]
        g[f"bought_a{act}"].append(sum(len(c["potions_in"]) for c in C if c["room"] == "Shop"))
        g[f"got_a{act}"].append(sum(len(c["potions_in"]) for c in C if c["room"] != "Shop"))
        g[f"skipped_pot_a{act}"].append(sum(1 for c in C if c["a"].startswith("SkipReward") and c["d"]=="Rewards" and not c["cards_in"]))
    shops3 = [c for c in L if c["t"] == "choice" and c["act"] == 3 and c["room"] == "Shop"]
    g["potion_gold_a3"].append(-sum(c["gold"] for c in shops3 if c["potions_in"]))
    g["boss_potions"].append(len(last["potions"]))
    g["boss_gold"].append(last["gold"])
    g["boss_used"].append(len(last["potions"]) - len(last.get("potions_after", [])) if "potions_after" in last else -1)
print("%-18s %8s %8s" % ("", "won", "lost"))
for k in agg["won"]:
    print("%-18s %8.2f %8.2f" % (k, st.mean(agg["won"][k]), st.mean(agg["lost"][k])))
print("n", len(agg["won"]["boss_gold"]), len(agg["lost"]["boss_gold"]))
