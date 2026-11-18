"""RLN39 §1: where eval runs die (act x fight type), top killers, capped runs.

    python python/rln39_deaths.py EVAL.jsonl [EVAL2.jsonl ...]
"""

import collections
import json
import sys


def kind(enc):
    for k in ("BOSS", "ELITE", "NORMAL", "WEAK"):
        if enc.endswith("_" + k):
            return "boss" if k == "BOSS" else "elite" if k == "ELITE" else "hallway"
    return "other"


def load(path):
    with open(path) as f:
        return [json.loads(line) for line in f][1:]


def main(paths):
    for p in paths:
        runs = load(p)
        died = [r for r in runs if r["end"] == "died"]
        ends = collections.Counter(r["end"] for r in runs)
        n = len(died)
        print(f"## {p.split('/')[-1]}: {len(runs)} runs, {dict(ends)}, mean floor "
              f"{sum(r['floor'] for r in runs) / len(runs):.1f}")
        where = collections.Counter()
        for r in died:
            if r["death"] is None:
                where[("?", "outside fight")] += 1
                continue
            where[(r["fight_acts"][-1], kind(r["death"]))] += 1
        print("\n| where | deaths | share |\n|---|---:|---:|")
        for (a, k), v in sorted(where.items(), key=lambda kv: str(kv[0])):
            print(f"| act {a} {k} | {v} | {100 * v / n:.1f}% |")
        print("\n| killer | act | deaths | share |\n|---|---:|---:|---:|")
        top = collections.Counter(r["death"] for r in died)
        for k, v in top.most_common(10):
            act = collections.Counter(r["fight_acts"][-1] for r in died if r["death"] == k).most_common(1)[0][0]
            print(f"| {k} | {act} | {v} | {100 * v / n:.1f}% |")
        cap = [r for r in runs if r["end"] == "capped"]
        if cap:
            fl = collections.Counter(r["fight_acts"][-1] if r["fight_acts"] else 0 for r in cap)
            print(f"\ncapped: {len(cap)} (last fight act {dict(fl)}), mean floor "
                  f"{sum(r['floor'] for r in cap) / len(cap):.1f}; last fights "
                  f"{collections.Counter(r['fights'][-1] for r in cap if r['fights']).most_common(5)}")
        print()


if __name__ == "__main__":
    main(sys.argv[1:])
