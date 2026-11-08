#!/usr/bin/env python3
"""Builds crates/sts2-diff/act_loadouts/<act>-<character>.json from the community dump (docs/history-replay.md).

Usage: act_loadouts.py <hive|glory> [--db PATH] [--bin target/regress/sts2-diff] [--write]

Takes the v0.111.0 solo runs that finished the act before (Hive: Act 1, Glory: Act 2; ascension >= 5, a seed, not
abandoned), reads each run's state at that act's last map point (HP, max HP, gold, deck and relics whose
`floor_added_to_deck` is up to there; upgrade levels are the final ones, the file keeps no history), and per character picks
the run nearest the medians of deck size, relic count, gold, HP and max HP. A candidate whose cards or relics the engine does
not port (`sts2-diff run-gen` stops with `loadout ...`) is skipped for the next nearest. The loadout removes the starter
cards the run has, adds the whole deck back (`ASCENDERS_BANE` is left to the engine) and adds the relics except the starter
relic (a run without it is not a candidate). Prints each pick; with --write the files are written. Never commit the dump
or runs from it: the loadouts hold one deck, a gold amount, HP and relic ids.
"""
import argparse
import json
import os
import sqlite3
import shutil
import statistics
import subprocess
import sys
import tempfile
from collections import Counter
from pathlib import Path


def cache_dir(name):
    """`~/.cache/spirezero-<name>`, or the old `~/.cache/spirebot-<name>` while only that exists."""
    new, old = os.path.expanduser(f"~/.cache/spirezero-{name}"), os.path.expanduser(f"~/.cache/spirebot-{name}")
    return old if not os.path.exists(new) and os.path.isdir(old) else new

CHARACTERS = ["Necrobinder", "Ironclad", "Silent", "Defect", "Regent"]
ACT_AFTER = {"hive": 0, "glory": 1}  # the act whose last map point is the loadout's state
ACT_INDEX = {"hive": 1, "glory": 2}
MIN_ASCENSION = 5

ap = argparse.ArgumentParser()
ap.add_argument("act", choices=ACT_AFTER)
ap.add_argument("--db", default=os.path.join(cache_dir("data"), "sts2fun/sts2_public.db"))
ap.add_argument("--bin", default="target/regress/sts2-diff")
ap.add_argument("--write", action="store_true")
args = ap.parse_args()
root = Path(__file__).resolve().parent.parent
done_act = ACT_AFTER[args.act]


def state(raw):
    """(hp, max_hp, gold, deck ids, relic ids) at the end of the act before `args.act`, or None."""
    run = json.loads(raw)
    history = run["map_point_history"]
    if len(history) <= done_act + 1 or run.get("modifiers") or len(run["players"]) != 1:
        return None
    floors = sum(len(a) for a in history[: done_act + 1])
    last = history[done_act][-1]
    if last["map_point_type"] != "boss":
        return None
    stats = last["player_stats"][0]
    player = run["players"][0]
    deck = [(c["id"].removeprefix("CARD."), c.get("current_upgrade_level", 0), c.get("enchantment"))
            for c in player["deck"] if c["floor_added_to_deck"] <= floors]
    relics = [r["id"].removeprefix("RELIC.") for r in player["relics"] if r["floor_added_to_deck"] <= floors]
    starters = {c["id"].removeprefix("CARD.") for c in player["deck"] if c["floor_added_to_deck"] <= 1}
    return (stats["current_hp"], stats["max_hp"], stats["current_gold"], deck, relics,
            player["character"].removeprefix("CHARACTER."), starters)


def card_id(card):
    name, upgrades, enchantment = card
    return name + (f"+{upgrades}" if upgrades else "")


db = sqlite3.connect(f"file:{args.db}?mode=ro", uri=True)
rows = db.execute(
    "select raw_json, seed from runs where build_id like '%0.111.0%' and ascension >= ? and not was_abandoned"
    " and seed is not null and seed != ''", (MIN_ASCENSION,))
by_character = {c: [] for c in CHARACTERS}
for raw, seed in rows:
    s = state(raw)
    if s and s[5].title() in by_character:
        by_character[s[5].title()].append(s)

work = Path(tempfile.mkdtemp(prefix="act-loadouts-", dir=root / "target"))
for character in CHARACTERS:
    runs = by_character[character]
    if not runs:
        sys.exit(f"{character}: no runs")
    # Starter cards and relic: what most runs hold from floor 1 (Neow's picks vary).
    first = Counter(c for run in runs for c in run[6])
    starter_cards = {c for c, n in first.items() if n > 0.5 * len(runs)} - {"ASCENDERS_BANE"}
    relic_count = Counter(r for run in runs for r in set(run[4]))
    starter_relics = {r for r, n in relic_count.items() if n > 0.95 * len(runs)}
    medians = [statistics.median(v) for v in (
        [len(r[3]) for r in runs], [len(r[4]) for r in runs], [r[2] for r in runs], [r[0] for r in runs], [r[1] for r in runs])]

    def distance(r):
        values = [len(r[3]), len(r[4]), r[2], r[0], r[1]]
        return sum(abs(v - m) / (m + 1) for v, m in zip(values, medians))

    picked = None
    for hp, max_hp, gold, deck, relics, *_ in sorted(runs, key=distance)[:200]:
        if not starter_relics <= set(relics) or any(c[2] for c in deck):
            continue
        cards = [c for c in deck if c[0] != "ASCENDERS_BANE"]
        added = [r for r in relics if r not in starter_relics]
        # Only the last relic of a loadout may open a prompt (Dolly's Mirror, ...): try each as the last.
        for last in range(len(added)):
            ordered = added[:last] + added[last + 1:] + [added[last]]
            loadout = {
                "act_index": ACT_INDEX[args.act],
                "add_cards": [card_id(c) for c in cards],
                "remove_cards": sorted(starter_cards & {c[0] for c in cards}),
                "add_relics": ordered,
                "gold": gold, "max_hp": max_hp, "hp": hp,
            }
            path = work / f"{character}.json"
            path.write_text(json.dumps(loadout))
            stop = subprocess.run(
                [root / args.bin, "run-gen", "--seed", "ACTLOAD0", "--policy-seed", "0", "--max-actions", "50", "--policy", "greedy",
                 "--ascension", "10", "--character", character, "--run-loadout", path, "--hits", work / "h.json",
                 "--actions", work / "a.json", "--trace", work / "t.jsonl"],
                capture_output=True, text=True).stdout
            if stop.startswith("stop=") and not stop.startswith("stop=loadout"):
                picked = loadout
                break
        if picked:
            break
    if picked is None:
        sys.exit(f"{character}: no run whose content the engine ports")
    print(f"{character}: {len(runs)} runs, medians deck/relics/gold/hp/max_hp {medians}; picked deck {len(picked['add_cards'])},"
          f" relics {len(picked['add_relics'])}, gold {picked['gold']}, hp {picked['hp']}/{picked['max_hp']}")
    if args.write:
        out = root / "crates" / "sts2-diff" / "act_loadouts" / f"{args.act}-{character.lower()}.json"
        out.write_text(json.dumps(picked) + "\n")
        print(f"  wrote {out.relative_to(root)}")
shutil.rmtree(work)
