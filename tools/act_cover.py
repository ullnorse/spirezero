#!/usr/bin/env python3
"""Builds crates/sts2-diff/act_loadouts/cover/<act>-<character>-<NN>.json: the cover loadouts of the act-jump passes.

Usage: act_cover.py <hive|glory> [--bin target/regress/sts2-diff] [--cards N] [--relics N] [--write]

The median-run loadout of each character (tools/act_loadouts.py) holds one deck, so the Hive and Glory zero-hit checks
(docs/hits-allowlist.md, `cargo xtask hits --acts <act>`) cannot reach most of the act's card and relic pool. The cover
loadouts hold the rest: per character, every card of its pool (and, shared out round-robin among the characters, the
colorless/curse/status/event cards and dependency tokens) and every relic of its pool or reachable only through events,
ancients or the Fake Merchant, in chunks of `--cards` cards and `--relics` relics on top of a full-HP, 400-gold run (the
character's starter deck is removed, so a chunk's cards are drawn and played). A chunk is checked with `sts2-diff
run-gen` (a stop `loadout ...` or an error means the engine cannot take it, e.g. a relic that prompts must come last):
each prompting relic goes last in a chunk of its own kind (at most one per chunk), a failing chunk is split in halves; what still fails alone is printed as unloadable and left out.
Also writes one forced-event loadout per event in EVENTS (`"event"` plus `"act_index"`, the run stops back on the map
after the event), the events the natural runs rarely enter, and one `ancient` loadout per character (the act's ancient
takes option <seed index> % <options>; with 50 seeds every option of the ancient the seed rolled is taken at least once). The hit files of the passes that run these are what the
act's zero-hit check reads (tools/final_matrix.sh).
"""
import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path

PROBE = "ANCHOR"  # a relic that never prompts
CHARACTERS = ["Necrobinder", "Ironclad", "Silent", "Defect", "Regent"]
ACT_INDEX = {"hive": 1, "glory": 2}
ACT_NAME = {"hive": "Hive", "glory": "Glory"}
STARTERS = {
    "Necrobinder": ["STRIKE_NECROBINDER", "DEFEND_NECROBINDER", "BODYGUARD", "UNLEASH"],
    "Ironclad": ["STRIKE_IRONCLAD", "DEFEND_IRONCLAD", "BASH"],
    "Silent": ["STRIKE_SILENT", "DEFEND_SILENT", "NEUTRALIZE", "SURVIVOR"],
    "Defect": ["STRIKE_DEFECT", "DEFEND_DEFECT", "ZAP", "DUALCAST"],
    "Regent": ["STRIKE_REGENT", "DEFEND_REGENT", "FALLING_STAR", "VENERATE"],
}
# The act's events that natural runs rarely enter (the zero-hit lists of a 50-seed pass), forced one per loadout.
EVENTS = {
    "hive": ["MYSTERIOUS_KNIGHT", "THE_LEGENDS_WERE_TRUE", "WAR_HISTORIAN_REPY"],
    "glory": ["BRAIN_LEECH", "DOLL_ROOM", "ROOM_FULL_OF_CHEESE", "STONE_OF_ALL_TIME", "TEA_MASTER",
              "THE_LEGENDS_WERE_TRUE", "WAR_HISTORIAN_REPY", "WELCOME_TO_WONGOS"],
}
EVENT_CHARACTER = "Silent"

ap = argparse.ArgumentParser()
ap.add_argument("act", choices=ACT_INDEX)
ap.add_argument("--bin", default="target/regress/sts2-diff")
ap.add_argument("--cards", type=int, default=10)
ap.add_argument("--relics", type=int, default=6)
ap.add_argument("--write", action="store_true")
args = ap.parse_args()
root = Path(__file__).resolve().parent.parent
work = Path(tempfile.mkdtemp(prefix="act-cover-", dir=root / "target"))


def snake(name):
    out = ""
    for i, c in enumerate(name):
        if c.isupper() and i and (not name[i - 1].isupper() or (i + 1 < len(name) and name[i + 1].islower())):
            out += "_"
        out += c.upper()
    return out


def inventory(character):
    return json.loads((root / "docs" / "inventory" / f"{character.lower()}-{args.act}.json").read_text())


def owners(kind, root_pool_suffix):
    """{id: characters whose inventory lists it}, and the ids in scope for no character's own pool."""
    listed, own = {}, {}
    for character in CHARACTERS:
        for e in inventory(character)["entities"]:
            if e["kind"] != kind or e["multiplayer_only"]:
                continue
            listed.setdefault(e["id"], []).append(character)
            if f"{character}{root_pool_suffix}" in e["pool"]:
                own.setdefault(e["id"], []).append(character)
    return listed, own


def assign(kind, suffix):
    """{character: sorted ids}: an entity of one character's pool goes to that character, the rest round-robin."""
    listed, own = owners(kind, suffix)
    out = {c: [] for c in CHARACTERS}
    shared = []
    for id_ in sorted(listed):
        if id_ in own and len(own[id_]) == 1:
            out[own[id_][0]].append(id_)
        else:
            shared.append(id_)
    for i, id_ in enumerate(shared):
        out[CHARACTERS[i % len(CHARACTERS)]].append(id_)
    return out


def loadout(character, cards, relics):
    data = {
        "act_index": ACT_INDEX[args.act],
        "add_cards": [snake(c) for c in cards],
        "remove_cards": STARTERS[character],
        "add_relics": relics,
        "gold": 400,
    }
    if not relics or relics[-1] not in prompting:
        data |= {"max_hp": 150, "hp": 150}  # the engine refuses hp after a relic that prompts
    return data


def loads(character, data):
    """Whether the engine takes `data` as a run loadout (no `stop=loadout`, no error)."""
    path = work / "l.json"
    path.write_text(json.dumps(data))
    run = subprocess.run(
        [root / args.bin, "run-gen", "--seed", "COVER0", "--policy-seed", "0", "--max-actions", "30", "--policy", "greedy",
         "--ascension", "0", "--character", character, "--run-loadout", path, "--hits", work / "h.json",
         "--actions", work / "a.json", "--trace", work / "t.jsonl"],
        capture_output=True, text=True)
    return run.returncode == 0 and run.stdout.startswith("stop=") and not run.stdout.startswith("stop=loadout"), run.stdout + run.stderr


prompting = set()


def find_prompting(character, ids, unloadable):
    """The relics of `ids` that open a prompt (one followed by another relic is refused: `follows a relic that prompts`);
    a relic the engine refuses for another reason goes to `unloadable`. Returns the others."""
    usable = []
    for r in ids:
        ok, text = loads(character, loadout(character, [], [r, PROBE]))
        if "follows a relic that prompts" in text:
            prompting.add(r)
        elif not ok:
            unloadable.append((character, r, text.strip().replace("\n", " ")[:160]))
            continue
        usable.append(r)
    return usable


def place(character, cards, relics, unloadable):
    """Loadouts holding `cards` and `relics` (a list: it is split until the engine takes the pieces)."""
    if not cards and not relics:
        return []
    data = loadout(character, cards, relics)
    ok, text = loads(character, data)
    if ok:
        return [data]
    if len(cards) + len(relics) == 1:
        unloadable.append((character, (cards + relics)[0], text.strip().replace("\n", " ")[:160]))
        return []
    ch, rl = len(cards) // 2, len(relics) // 2
    if ch + rl == 0:
        ch = len(cards)
    left = place(character, cards[:ch], relics[:rl], unloadable)
    return left + place(character, cards[ch:], relics[rl:], unloadable)


cards = assign("card", "CardPool")
relics = assign("relic", "RelicPool")
result = {}
unloadable = []
for character in CHARACTERS:
    cs, rs = cards[character], [snake(r) for r in relics[character]]
    rs = find_prompting(character, rs, unloadable)
    plain, prompts = [r for r in rs if r not in prompting], [r for r in rs if r in prompting]
    n = max(-(-len(cs) // args.cards), -(-len(plain) // args.relics), len(prompts))
    loadouts = []
    for i in range(n):
        chunk = plain[i * args.relics:(i + 1) * args.relics] + prompts[i:i + 1]
        loadouts += place(character, cs[i * args.cards:(i + 1) * args.cards], chunk, unloadable)
    result[character] = loadouts
    print(f"{character}: {len(cs)} cards, {len(rs)} relics ({len(prompts)} prompt) -> {len(loadouts)} loadouts", file=sys.stderr)
events = []
for event in EVENTS[args.act]:
    data = {"event": event, "act_index": ACT_INDEX[args.act], "max_hp": 150, "hp": 150, "gold": 400}
    ok, text = loads(EVENT_CHARACTER, data)
    print(f"event {event}: {'ok' if ok else 'unloadable: ' + text.strip()[:200]}", file=sys.stderr)
    if ok:
        events.append((event, data))
ancients = {c: {"act_index": ACT_INDEX[args.act], "ancient": True} for c in CHARACTERS}
for character, entity, why in unloadable:
    print(f"unloadable: {character} {entity}: {why}", file=sys.stderr)

if args.write:
    out = root / "crates" / "sts2-diff" / "act_loadouts" / "cover"
    out.mkdir(parents=True, exist_ok=True)
    for old in out.glob(f"{args.act}-*.json"):
        old.unlink()
    for character, loadouts in result.items():
        for i, data in enumerate(loadouts):
            (out / f"{args.act}-{character.lower()}-{i:02}.json").write_text(json.dumps(data) + "\n")
    for event, data in events:
        (out / f"{args.act}-{EVENT_CHARACTER.lower()}-event-{event.lower().replace('_', '-')}.json").write_text(json.dumps(data) + "\n")
    for character, data in ancients.items():
        (out / f"{args.act}-{character.lower()}-ancient.json").write_text(json.dumps(data) + "\n")
    print(f"wrote {sum(map(len, result.values())) + len(events) + len(ancients)} loadouts to {out.relative_to(root)}", file=sys.stderr)
import shutil
shutil.rmtree(work)
