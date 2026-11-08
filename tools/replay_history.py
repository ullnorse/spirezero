#!/usr/bin/env python3
"""Check the engine against recorded `.run` files (docs/PLAN.md "Truth sources", docs/history-replay.md).

Usage: tools/replay_history.py [--history DIR ...] [--db [PATH]] [--build v0.111.0] [--character ALL|NAME]
                               [--act1 all|overgrowth|underdocks] [--sample N] [--run SEED] [--jobs N]
                               [--out FILE] [--summary FILE] [--verbose] [--reuse]

Sources (read-only; nothing is copied into the repo):
  --history DIR  `*.run` files (default, if no --db: the user's Steam profile `history/` and `modded/.../history/`)
  --db [PATH]    the community dump (default ~/.cache/spirezero-data/sts2fun/sts2_public.db), `runs.raw_json`
                 with build_id, n_players=1, not has_modifiers, is_valid and a seed
Runs are filtered to the build, standard mode, no modifiers and one player; --sample N keeps a fixed
pseudo-random N per character x Act 1; --run SEED keeps only that seed's runs and prints each check
(a repro). Runs abandoned before Neow are skipped.

Each run's seed and recorded path go to `sts2-diff history-predict --batch` (the engine forced to the
run's character and recorded Act 1), which reports:
  act1        the engine's own Act 1 roll (`act_selection`) against the recorded act
  map, unknown_room, encounter, boss, ancient, event_order
              per act (Acts 1-3; at A10 Glory's second boss too): a map path with the recorded point types, the
              `?` rolls, the normal/elite encounter queue, the boss, the act's ancient, and events in
              the act list's order (first mismatch per act; later floors of that act are unchecked)
  neow_options, neow_rewards, neow_cards, event, card_reward, potion_reward, relic_reward, combat_gold,
  treasure_relic, treasure_gold, shop_cards, shop_relics, shop_potions, dig_relic
              Act 1 floor by floor (crates/sts2-diff/src/history_replay.rs): the generators run on a state
              fed the recorded gold, HP, deck, belt and relics. After an unmodeled `rewards` consumer (an
              event) the next fight or shop is matched by searching the gap's draw count ("resynced").
Every mismatch is classified (`classify`): profile (unlock and discovery state, detected by rule),
file (what the format can't pin), engine (an engine gap or bug). The summary goes to --summary and
the per-run details to --out (both under target/ or ~/.cache/spirezero-data/; never commit run data).
"""
import argparse
import collections
import functools
import glob
import json
import os
import random
import re
import sqlite3
import subprocess
import sys
from pathlib import Path


def cache_dir(name):
    """`~/.cache/spirezero-<name>`, or the old `~/.cache/spirebot-<name>` while only that exists."""
    new, old = os.path.expanduser(f"~/.cache/spirezero-{name}"), os.path.expanduser(f"~/.cache/spirebot-{name}")
    return old if not os.path.exists(new) and os.path.isdir(old) else new

ROOT = Path(__file__).resolve().parent.parent
# The game's per-account save dir (steam/<steam id>); STS2_STEAM overrides, else the first one found.
STEAM = Path(os.environ["STS2_STEAM"]) if "STS2_STEAM" in os.environ else next(
    iter(sorted((Path.home() / ".local/share/SlayTheSpire2/steam").glob("*"))), Path("/nonexistent"))
DEFAULT_DIRS = [STEAM / "profile1/saves/history", STEAM / "modded/profile1/saves/history"]
DEFAULT_DB = Path(cache_dir("data")) / "sts2fun/sts2_public.db"
REF = Path(os.environ.get("STS2_REF", ROOT / "ref"))
BIN = ROOT / "target/regress/sts2-diff"
CHARACTERS = ["IRONCLAD", "SILENT", "DEFECT", "REGENT", "NECROBINDER"]
ACTS = ["overgrowth", "underdocks"]
WALK_KINDS = {"no_map_path": "map", "room_type": "unknown_room", "encounter": "encounter",
              "ancient": "ancient", "event_not_in_pool": "event_order", "event_order": "event_order",
              "map_error": "map"}
CHECKS = ["act1", "map", "unknown_room", "encounter", "boss", "ancient", "event_order", "event",
          "neow_options", "neow_offers", "neow_relics", "card_reward", "potion_reward", "relic_reward",
          "combat_gold", "treasure_relic", "treasure_gold", "shop_cards", "shop_relics",
          "shop_potions", "dig_relic"]


def bare(model_id):
    return model_id.split(".", 1)[-1] if model_id else model_id


# ---------------------------------------------------------------------------------------------
# Sources


def runs_from_history(dirs):
    for d in dirs:
        for f in sorted(glob.glob(str(d / "*.run"))):
            yield {"source": Path(f).name, "player": "local"}, json.loads(Path(f).read_text())


def runs_from_db(path, build):
    db = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    q = ("select id, player_id, raw_json from runs where build_id = ? and n_players = 1 "
         "and not has_modifiers and is_valid and seed is not null")
    for rid, player, raw in db.execute(q, (build,)):
        yield {"source": f"db:{rid}", "player": str(player)}, json.loads(raw)


def usable(run, build):
    return (run.get("build_id") == build and run.get("game_mode") == "standard" and not run.get("modifiers")
            and len(run.get("players", [])) == 1 and run.get("seed")
            and run.get("map_point_history") and run["map_point_history"][0])  # not abandoned before Neow


# ---------------------------------------------------------------------------------------------
# Profile state: which epochs a player had revealed (docs/specs/test-mode.md #4/#5, ActModel.GetRandomList)


def epoch_content():
    """{epoch id: set of gated ids (`CARD.X`, `RELIC.X`, `POTION.X`, `EVENT.X`)} from the decomp's
    Timeline/Epochs (read at run time, never stored), restricted to what pools and act event lists filter."""
    d = REF / "decomp/MegaCrit.Sts2.Core.Timeline.Epochs"
    if not d.is_dir():
        return {}
    kinds = {"Card": "CARD", "Relic": "RELIC", "Potion": "POTION", "Event": "EVENT", "AncientEvent": "EVENT"}
    out = {}
    for f in sorted(d.glob("*.cs")):
        text = f.read_text()
        m = re.search(r'Id => "([A-Z0-9_]+)"', text)
        if not m:
            continue
        ids = {f"{kinds[k]}.{re.sub(r'(?<!^)(?=[A-Z])', '_', n).upper()}"
               for k, n in re.findall(r"ModelDb\.(Card|Relic|Potion|Event|AncientEvent)<(\w+)>\(\)", text)}
        if ids:
            out[m.group(1)] = ids
    # Not lists in the epoch classes: `UnlockState.SharedAncients` and `Underdocks.IsUnlocked`.
    out["DARV_EPOCH"] = {"EVENT.DARV"}
    out["UNDERDOCKS_EPOCH"] = {"ACT.UNDERDOCKS"}
    return out


def run_content(run):
    """Every card, relic, potion and event id the run shows anywhere."""
    seen = set(run.get("acts", []))
    for p in run["players"]:
        seen.update(c["id"] for c in p.get("deck", []))
        seen.update(r["id"] for r in p.get("relics", []))
        seen.update(x["id"] for x in p.get("potions", []) if isinstance(x, dict) and "id" in x)
    for act in run["map_point_history"]:
        for e in act:
            for r in e.get("rooms", []):
                if r.get("model_id"):
                    seen.add(r["model_id"])
            for s in e["player_stats"]:
                seen.update(c["card"]["id"] for c in s.get("card_choices", []))
                seen.update(c["choice"] for c in s.get("relic_choices", []) + s.get("potion_choices", []))
                seen.update(c["id"] for c in s.get("cards_gained", []))
                seen.update("RELIC." + c["TextKey"] for c in s.get("ancient_choice", []))
    return seen


# A pseudo-epoch per character, revealed by a solo run as it: `UnlockState.Characters` drops Silent,
# Regent, Necrobinder and Defect until their `<Name>1Epoch` is revealed (Ironclad is always there).
PLAYED = "PLAYED."
LOCKABLE_CHARACTERS = ["SILENT", "DEFECT", "REGENT", "NECROBINDER"]
# Mismatches whose cause is profile state that no rule here can see, with the evidence (docs/history-replay.md).
PROFILE_CASES = {
    ("LNWW8D4WDQ5N", "shop_cards"): "Colorless2Epoch not revealed (another profile?): the colorless uncommon "
                                    "and rare picks both reproduce without PrepTime, EternalArmor and Jackpot",
    ("C5065PT737SC", "shop_cards"): "Colorless2Epoch not revealed (another profile?): the colorless uncommon "
                                    "and rare picks both reproduce without PrepTime, EternalArmor and Jackpot",
}

# Uploaders whose v0.111.0 client disagrees with the decomp on independent streams (docs/history-replay.md
# "Engine-bug candidates", PX19): their mismatches are classed `player`, not engine.
CLIENT_CASES = {
    "10001215": "client artifact: 13 of the uploader's 30 runs miss on the `?` and chest streams at once "
                "(e.g. HULYAJEMURRQ f13: the next `?` draw is 0.806, an event at any decomp odds; "
                "SGHSU9TF1NAE skips ORICHALCUM at two chests, then gets it)",
}


def revealed_times(sources, build_filter, epochs):
    """{player: {epoch: earliest start_time a run of theirs (any build) shows the epoch's content}}."""
    out = collections.defaultdict(dict)
    for meta, run in sources:
        if build_filter is not None and not build_filter(meta["player"]):
            continue
        content = run_content(run)
        if len(run.get("players", [])) == 1:
            # Only a solo run shows which character the uploader could pick.
            content.add(PLAYED + character_of(run))
        t = run.get("start_time", 0)
        for epoch, ids in [*epochs.items(), *((PLAYED + c, {PLAYED + c}) for c in LOCKABLE_CHARACTERS)]:
            if content & ids:
                cur = out[meta["player"]].get(epoch)
                if cur is None or t < cur:
                    out[meta["player"]][epoch] = t
    return out


def relevant_epochs(epochs, character):
    """Epochs whose content the run's pools can draw: shared ones and the character's own."""
    others = [c for c in CHARACTERS if c != character]
    return [e for e in epochs if not any(e.startswith(o) for o in others)]


# ---------------------------------------------------------------------------------------------
# Engine


def digest(run, titles=None):
    """The seed and, per act, each floor's point type, first room and relics held on entering.

    Floor numbers count entries over all acts from 1 (Neow); a relic with `floor_added_to_deck` f
    is held by every room from floor f + 1. `spoils[a]`: a Spoils Map card was in the deck (gained
    and not removed) when act a began, which swaps that act's map."""
    melted = melt_floors(run)
    relics = [(r["id"], r.get("floor_added_to_deck", 1), melted.get(i))
              for i, r in enumerate(run["players"][0]["relics"])]
    relics += removed_relics(run)
    acts, spoils, floor, quest, lantern = [], [], 0, 0, 0
    for act in run["map_point_history"]:
        spoils.append(quest > 0)
        entries = []
        for e in act:
            floor += 1
            rooms = e.get("rooms") or [{}]
            entries.append({"point": e["map_point_type"], "room": rooms[0].get("room_type", ""),
                            "model": rooms[0].get("model_id"),
                            "lantern": lantern > 0,
                            "relics": [i for i, f, r in relics if f < floor and (r is None or floor <= r)]})
            for p in e["player_stats"]:
                lantern += sum(c["id"] == "CARD.LANTERN_KEY" for c in p.get("cards_gained", []))
                lantern -= sum(c["id"] == "CARD.LANTERN_KEY" for c in p.get("cards_removed", []))
                quest += sum(c["id"] == "CARD.SPOILS_MAP" for c in p.get("cards_gained", []))
                quest -= sum(c["id"] == "CARD.SPOILS_MAP" for c in p.get("cards_removed", []))
        acts.append(entries)
    golden = next((r.get("floor_added_to_deck", 1) for r in run["players"][0]["relics"]
                   if r["id"] == "RELIC.GOLDEN_COMPASS"), None)
    golden_act = None
    if golden is not None:  # the act whose entries include the floor it was taken on
        total = 0
        for a, act in enumerate(run["map_point_history"]):
            total += len(act)
            if golden <= total:
                golden_act = a
                break
    return {"seed": run["seed"], "ascension": run["ascension"], "spoils": spoils, "golden_act": golden_act,
            "acts": acts,
            "act1": act1_of(run), "character": character_of(run).capitalize(),
            "run": with_hidden_pulls(run, titles or {})}


def melt_floors(run):
    """{index in the final relic list: floor it melted on} for `ToyBox`'s wax relics recorded as
    `IsMelted`. `ToyBox.AfterCombatEnd` melts the first wax relic not yet melted (relic order) after
    every 3rd combat from the one it was taken on, and a melted relic is no hook listener
    (`RunState.IterateHookListeners` skips `IsMelted`), so a melted `JuzuBracelet` lets `?` rooms
    roll fights again. The file has `IsWax`/`IsMelted`, not the floor; it is counted from the rooms."""
    def flag(r, name):
        return any(b["name"] == name and b["value"] for b in r.get("props", {}).get("bools", []))
    relics = run["players"][0]["relics"]
    box = next((r.get("floor_added_to_deck", 1) for r in relics if r["id"] == "RELIC.TOY_BOX"), None)
    if box is None:
        return {}
    melts, floor, combats = [], 0, 0
    for act in run["map_point_history"]:
        for e in act:
            floor += 1
            if floor <= box:
                continue
            for room in e.get("rooms", []):
                if room.get("room_type") in ("monster", "elite", "boss"):
                    combats += 1
                    if combats % 3 == 0:
                        melts.append(floor)
    wax = [i for i, r in enumerate(relics) if flag(r, "IsWax") and flag(r, "IsMelted")]
    return {i: melts[k] if k < len(melts) else floor for k, i in enumerate(wax)}


# Events that `PullNextRelicFromFront` relics the file only names by their (localised) title in the
# event's variables: `RelicTrader.NewRelics` (3, only the traded one is a relic choice) and
# `WelcomeToWongos.FeaturedItem` (pulled when the event opens, bought or not). The pulls leave both bags.
HIDDEN_PULLS = {"RELIC_TRADER": ("TopRelicNew", "MiddleRelicNew", "BottomRelicNew"),
                "WELCOME_TO_WONGOS": ("RandomRelic",)}


def hidden_pulls(entry, titles):
    """([relic ids], [unmapped titles]) an entry's event pulled without recording them."""
    ids, unknown = [], []
    for room in entry.get("rooms", []):
        keys = HIDDEN_PULLS.get(bare(room.get("model_id") or ""))
        if not keys:
            continue
        for p in entry["player_stats"]:
            for c in p.get("event_choices", []):
                for k in keys:
                    t = ((c.get("variables") or {}).get(k) or {}).get("string_value")
                    if not t:
                        continue
                    i = titles.get(t) or english_titles().get(re.sub(r"[^A-Z0-9]", "", t.upper()))
                    if i:
                        ids.append(i)
                    else:
                        unknown.append(t)
    return ids, unknown


def with_hidden_pulls(run, titles):
    """A copy of `run` whose floors carry `hidden_relic_pulls` (read by `history_replay.rs`)."""
    run = json.loads(json.dumps(run))
    for act in run["map_point_history"]:
        for e in act:
            ids, _ = hidden_pulls(e, titles)
            if ids and e["player_stats"]:
                e["player_stats"][0]["hidden_relic_pulls"] = ids
    return run


@functools.cache
def english_titles():
    """{title key: relic id} for English titles: the id is the title upper-cased, spaces to `_`,
    punctuation dropped (`Charon's Ashes` is `CHARONS_ASHES`). Ids from the decomp's index."""
    index = REF / "INDEX.tsv"
    if not index.exists():
        return {}
    out = {}
    for line in index.read_text().splitlines():
        kind, name, *_ = line.split("\t") + [""]
        if kind == "relic":
            out[name.upper()] = "RELIC." + re.sub(r"(?<!^)(?=[A-Z])", "_", name).upper()
    return out


def title_pairs(runs):
    """{localised title: relic id} from the runs themselves: the traded `RelicTrader` option's
    `<Pos>RelicNew`/`<Pos>RelicOwned` against the relic picked/removed, and a bought Wongo featured relic."""
    pairs = collections.defaultdict(collections.Counter)
    pos = {"TOP": "Top", "MIDDLE": "Middle", "BOTTOM": "Bottom"}
    for run in runs:
        for act in run.get("map_point_history", []):
            for e in act:
                for p in e.get("player_stats", []):
                    got = [x["choice"] for x in p.get("relic_choices", []) if x.get("was_picked")]
                    removed = p.get("relics_removed", [])
                    for c in p.get("event_choices", []):
                        key, v = c["title"]["key"], c.get("variables") or {}
                        part = key.split(".")
                        if key.startswith("RELIC_TRADER.pages.INITIAL.options.") and part[4] in pos:
                            for var, ids in ((pos[part[4]] + "RelicNew", got), (pos[part[4]] + "RelicOwned", removed)):
                                if len(ids) == 1 and (v.get(var) or {}).get("string_value"):
                                    pairs[v[var]["string_value"]][ids[0]] += 1
                        if key == "WELCOME_TO_WONGOS.pages.INITIAL.options.FEATURED_ITEM.title" and len(got) == 1 \
                                and (v.get("RandomRelic") or {}).get("string_value"):
                            pairs[v["RandomRelic"]["string_value"]][got[0]] += 1
    return {t: c.most_common(1)[0][0] for t, c in pairs.items()}



def removed_relics(run):
    """[(id, floor gained, floor removed)] of relics the run lost (`relics_removed`), which the final
    relic list no longer has: held from the floor after the last pick or purchase up to the removal."""
    out, gained, floor = [], {}, 0
    for act in run["map_point_history"]:
        for e in act:
            floor += 1
            for p in e["player_stats"]:
                for c in p.get("relic_choices", []):
                    if c.get("was_picked"):
                        gained[c["choice"]] = floor
                for i in p.get("bought_relics", []):
                    gained[i] = floor
                for i in p.get("relics_removed", []):
                    out.append((i, gained.pop(i, 1), floor))
    return out


def act1_of(run):
    return bare(run["acts"][0]).lower()


def character_of(run):
    return bare(run["players"][0]["character"])


def predict_all(runs, jobs, titles):
    """`history-predict --batch` over `runs`, split across `jobs` processes, in order."""
    chunks = [runs[i::jobs] for i in range(jobs)]
    procs = []
    for i, chunk in enumerate(chunks):
        inp, out = ROOT / f"target/history-replay-in-{i}.jsonl", ROOT / f"target/history-replay-out-{i}.jsonl"
        with open(inp, "w") as f:
            for _, run in chunk:
                f.write(json.dumps(digest(run, titles)) + "\n")
        with open(inp) as fin, open(out, "w") as fout:
            procs.append((inp, out, subprocess.Popen([str(BIN), "history-predict", "--batch"], stdin=fin,
                                                     stdout=fout, stderr=subprocess.PIPE, text=True)))
    results = [None] * len(runs)
    for i, (inp, out, p) in enumerate(procs):
        _, err = p.communicate()
        lines = out.read_text().splitlines()
        if p.returncode != 0 or len(lines) != len(chunks[i]):
            sys.exit(f"history-predict failed ({p.returncode}): {err.strip()[:500]}")
        for j, line in enumerate(lines):
            results[i + j * jobs] = json.loads(line)
        inp.unlink()
        out.unlink()
    return results


def first_run_fits(runs, results):
    """Indices of the runs with a `?` room mismatch whose every act's room types match when walked as
    the profile's first run (`NumberOfRuns == 0`: the first three `?` rooms draw nothing). Such a run
    also has the first-run Act 1 order and no epoch revealed, so it is all profile state."""
    todo = [i for i, r in enumerate(results)
            if any(a.get("mismatch", {}).get("kind") == "room_type" for a in r.get("acts", []))]
    if not todo:
        return set()
    lines = []
    for i in todo:
        d = digest(runs[i][1])
        del d["run"]
        lines.append(json.dumps({**d, "first_run": True, "types_only": True}))
    out = subprocess.run([str(BIN), "history-predict", "--batch"], input="\n".join(lines) + "\n",
                         capture_output=True, text=True, check=True).stdout.splitlines()
    return {i for i, line in zip(todo, out)
            if all(a["status"] in ("match", "unsupported") for a in json.loads(line).get("acts", []))}


# ---------------------------------------------------------------------------------------------
# Checks and classes


def outcomes(run, result):
    """[(check, status, detail)] for one run: status is match or mismatch; detail has floor/values."""
    out = []
    act1 = act1_of(run)
    if "act1_roll" in result:
        rolled = "underdocks" if result["act1_roll"] == 1 else "overgrowth"
        out.append(("act1", "match" if rolled == act1 else "mismatch", {"predicted": rolled, "recorded": act1}))
    for act in result.get("acts", []):
        if act["status"] == "unsupported":
            continue
        tag = {"act": act["act"] + 1}
        if act["status"] == "match":
            for c in ("map", "unknown_room", "encounter", "boss", "ancient", "event_order"):
                out.append((c, "match", tag))
            continue
        m = dict(act.get("mismatch", {}))
        if "floor" in m:  # act-local (the ancient is 0) to the run's floor number (Neow is 1)
            m["floor"] += 1 + sum(len(a) for a in run["map_point_history"][:act["act"]])
        kind = m.get("kind", act["status"])
        check = "boss" if kind == "encounter" and m.get("point") == "boss" else WALK_KINDS.get(kind, kind)
        out.append((check, "mismatch", {**tag, **m}))
        if check != "map":
            out.append(("map", "match", tag))
    for c in result.get("replay", {}).get("checks", []):
        out.append((c["field"], c["status"], c))
    return out


# Events whose `IsAllowed` reads the deck, belt, relics, HP or gold, which the file only lets us rebuild
# approximately (the state mid-floor, enchantable cards, relics a floor swapped).
DECK_EVENTS = {"BYRDONIS_NEST", "MORPHIC_GROVE", "WOOD_CARVINGS", "SLIPPERY_BRIDGE", "THE_FUTURE_OF_POTIONS",
               "SPIRALING_WHIRLPOOL", "STONE_OF_ALL_TIME", "THE_LEGENDS_WERE_TRUE", "AMALGAMATOR",
               "RANWID_THE_ELDER", "GRAVE_OF_THE_FORGOTTEN", "COLOSSAL_FLOWER", "RELIC_TRADER", "FAKE_MERCHANT"}
CHAIN = {"card_reward", "potion_reward", "relic_reward", "combat_gold", "treasure_gold", "shop_cards",
         "shop_relics", "shop_potions", "dig_relic", "neow_offers", "neow_relics"}


def multiset_le(a, b):
    return all(b.count(x) >= a.count(x) for x in set(a))


def classify(check, detail, ctx):
    """(class, reason) of a mismatch: profile (excluded by rule), player, file, or engine."""
    if ctx["modded"]:
        return "player", "modded client (mod content ids or BaseLib save fields)"
    if ctx["player"] in CLIENT_CASES:
        return "player", CLIENT_CASES[ctx["player"]]
    if ctx["first_run"]:
        return "profile", "the profile's first run (NumberOfRuns == 0): tutorial `?` rooms and Act 1 order, no epochs"
    if not re.fullmatch(r"[0-9A-Z]{12}", ctx["seed"]):
        return "file", "seed rewritten by the uploader (`old` prefix)"
    if check == "neow_options" and detail.get("recorded") == []:
        return "file", "no ancient_choice recorded (older save)"
    if ctx["first_point"] != "ancient":
        return "file", "no Neow floor recorded (the history starts at a fight)"
    if check == "act1":
        if detail["recorded"] == "underdocks":
            return "profile", "Underdocks undiscovered: GetRandomList forces it (ActModel.cs:550)"
        return "profile", "Underdocks locked (UnderdocksEpoch not revealed)"
    if check == "boss":
        return "profile", "boss discovery order (ApplyDiscoveryOrderModifications, test-mode #4)"
    if check in ("map", "unknown_room") and "WINGED_BOOTS" in ctx["relics"]:
        return "player", "Winged Boots: free travel off the map's edges"
    if check == "map" and "GOLDEN_COMPASS" in ctx["relics"]:
        return "engine", "gap: Golden Compass map not ported"
    if detail.get("unported_relics") and check not in ("encounter", "boss", "ancient", "event_order", "map"):
        return "engine", "gap: held relic not ported"
    if (ctx["seed"], check) in PROFILE_CASES:
        return "profile", PROFILE_CASES[ctx["seed"], check]
    if check in ("neow_options", "neow_relics") and not ctx["all_characters"]:
        return "profile", "Kaleidoscope (Neow, Neow's Bones) needs every character; no solo run proves it"
    if check == "neow_offers" and ctx["neow"] == "KALEIDOSCOPE" and not ctx["unlocks_proven_all"]:
        return "profile", "Kaleidoscope draws the other characters' pools: their epochs not all seen yet"
    if not ctx["unlocks_proven"] and check not in ("map", "unknown_room"):
        return "profile", "epoch content not seen by the player yet (pools may be locked)"
    if detail.get("resynced"):
        return "file", "after a resync over an unmodeled gap (chain uncertain)"
    rec, pred = detail.get("recorded"), detail.get("predicted")
    if check in CHAIN and detail.get("floor") == ctx["last_floor"] and (not rec or ctx["died_in_fight"]):
        # A fight the run died in has no reward screen; its potion_choices are procured potions.
        return "file", "the run ended on this floor (no rewards taken)"
    if (check in ("card_reward", "shop_cards") and isinstance(rec, list) and isinstance(pred, list)
            and sorted(c.rstrip("+") for c in rec) == sorted(c.rstrip("+") for c in pred)):
        return "file", "upgrades from a relic taken on the same screen (ModifyCardRewardForObtainedRelic, eggs)"
    if check == "shop_cards" and isinstance(rec, list) and multiset_le([c.rstrip("+") for c in pred],
                                                                       [c.rstrip("+") for c in rec]):
        return "file", "card_choices also has offers from a relic bought there (Orrery, ...)"
    if check == "shop_relics" and isinstance(rec, list) and multiset_le(pred, rec):
        return "file", "relic_choices also has the restocked entry (The Courier)"
    if (check in ("treasure_relic", "relic_reward", "shop_relics", "dig_relic") and ctx["unmapped_pull"]
            and (detail.get("floor") or 0) > ctx["unmapped_pull"]):
        return "file", "an event pulled a relic the file names by an unmapped localised title (hidden_pulls)"
    if check == "treasure_relic" and rec == []:
        return "file", "chest relic not taken, so not recorded"
    if check in ("combat_gold", "treasure_gold") and isinstance(rec, int) and rec > pred:
        return "file", "gold_gained also counts other gold"
    if check == "combat_gold" and isinstance(rec, int) and rec < pred:
        return "file", "escaped or thieving monsters (GoldProportion is not recorded)"
    if (check == "potion_reward" and isinstance(rec, list) and ctx["card_ok"].get(detail.get("floor"))
            and all(rec.count(p) >= pred.count(p) for p in pred)):
        return "file", "potion_choices also lists potions procured in the fight"
    if check == "event" and pred in DECK_EVENTS:
        return "file", "event IsAllowed reads the deck or belt (rebuilt approximately)"
    if check in ("card_reward", "potion_reward", "relic_reward", "combat_gold") and ctx["neow"] == "NEOWS_BONES":
        return "engine", "after Neow's Bones"
    return "engine", check


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--history", action="append", type=Path)
    ap.add_argument("--db", nargs="?", const=DEFAULT_DB, type=Path)
    ap.add_argument("--build", default="v0.111.0")
    ap.add_argument("--character", default="ALL")
    ap.add_argument("--act1", default="all")
    ap.add_argument("--sample", type=int)
    ap.add_argument("--run", help="replay only this seed and print every check and gap")
    ap.add_argument("--jobs", type=int, default=max(1, min(8, (os.cpu_count() or 2) - 1)))
    ap.add_argument("--out", type=Path, default=ROOT / "target/history-replay.json")
    ap.add_argument("--summary", type=Path, default=ROOT / "target/history-replay-summary.json")
    ap.add_argument("--verbose", action="store_true")
    ap.add_argument("--reuse", action="store_true", help="reuse the last run's predictions (same selection)")
    args = ap.parse_args()
    if not args.reuse:
        subprocess.run(["cargo", "build", "-q", "--profile", "regress", "-p", "sts2-diff"], cwd=ROOT, check=True)

    use_db = args.db is not None
    if use_db and not args.db.exists():
        print(f"no DB at {args.db}; skipped")
        return 0
    source = (lambda: runs_from_db(args.db, args.build)) if use_db else \
        (lambda: runs_from_history(args.history or DEFAULT_DIRS))
    seen, runs = set(), []
    for meta, run in source():
        key = (run.get("seed"), run.get("start_time"))
        if key in seen or not usable(run, args.build):
            continue
        if args.character != "ALL" and character_of(run) != args.character.upper():
            continue
        if args.act1 != "all" and act1_of(run) != args.act1:
            continue
        if args.run and run.get("seed") != args.run:
            continue
        seen.add(key)
        runs.append((meta, run))
    if args.sample:
        groups = collections.defaultdict(list)
        for r in runs:
            groups[character_of(r[1]), act1_of(r[1])].append(r)
        runs = []
        for k in sorted(groups):
            g = sorted(groups[k], key=lambda r: (r[1]["seed"], r[1]["start_time"]))
            runs += random.Random(f"{k}").sample(g, min(args.sample, len(g)))
    print(f"{len(runs)} {args.build} runs from {'the DB' if use_db else 'history dirs'}")

    # Profile evidence: every run of the same players, any build, from the same source.
    epochs = epoch_content()
    players = {m["player"] for m, _ in runs}
    if use_db:
        # Every player's evidence, cached next to the DB (derived data, never committed).
        cache = args.db.parent.parent / "h5/epoch-evidence.json"
        stamp = f"{args.db.stat().st_mtime}:{sorted(epochs)}:{LOCKABLE_CHARACTERS}"
        cached = json.loads(cache.read_text()) if cache.exists() else {}
        if cached.get("stamp") != stamp:
            db = sqlite3.connect(f"file:{args.db}?mode=ro", uri=True)
            ev = (({"player": str(p)}, json.loads(raw)) for p, raw in
                  db.execute("select player_id, raw_json from runs where n_players = 1"))
            cached = {"stamp": stamp, "revealed": revealed_times(ev, None, epochs)}
            cache.parent.mkdir(parents=True, exist_ok=True)
            cache.write_text(json.dumps(cached))
        revealed = cached["revealed"]
    else:
        ev = runs_from_history(args.history or DEFAULT_DIRS)
        revealed = revealed_times(ev, lambda p: p in players, epochs)

    # Localised relic titles (`hidden_pulls`), learned from every run of the source (derived, cached).
    if use_db:
        cache = args.db.parent.parent / "h5/relic-titles.json"
        stamp = f"{args.db.stat().st_mtime}"
        cached = json.loads(cache.read_text()) if cache.exists() else {}
        if cached.get("stamp") != stamp:
            db = sqlite3.connect(f"file:{args.db}?mode=ro", uri=True)
            cached = {"stamp": stamp, "titles": title_pairs(json.loads(raw) for (raw,) in db.execute(
                "select raw_json from runs where raw_json like '%RELIC_TRADER.pages%' "
                "or raw_json like '%WELCOME_TO_WONGOS.pages%'"))}
            cache.write_text(json.dumps(cached, ensure_ascii=False))
        titles = cached["titles"]
    else:
        titles = title_pairs(run for _, run in runs_from_history(args.history or DEFAULT_DIRS))

    # Raw predictions are kept (derived data, outside the repo) so --reuse can reclassify quickly.
    cache = Path(cache_dir("data")) / f"h5/predictions-{'db' if use_db else 'history'}.jsonl"
    if args.run:
        cache = ROOT / "target/history-replay-run.jsonl"
    if args.reuse and cache.exists():
        results = [json.loads(line) for line in cache.read_text().splitlines()]
        if len(results) != len(runs):
            sys.exit(f"--reuse: {cache} has {len(results)} results for {len(runs)} runs")
    else:
        results = predict_all(runs, args.jobs, titles)
        cache.parent.mkdir(parents=True, exist_ok=True)
        cache.write_text("".join(json.dumps(r) + "\n" for r in results))
    first_runs = first_run_fits(runs, results)
    stats = collections.Counter()  # (char, act, check, status)
    by_act = collections.Counter()  # (check, act number, status): the same counts per act of the run
    classes = collections.Counter()  # (class, check, reason)
    rules = collections.Counter()
    repro = collections.defaultdict(list)
    report = []
    for k, ((meta, run), result) in enumerate(zip(runs, results)):
        char, act1 = character_of(run), act1_of(run)
        if "error" in result:
            stats[char, act1, "error", "mismatch"] += 1
            classes["engine", "error", result["error"][:80]] += 1
            continue
        t = run.get("start_time", 0)
        rel = relevant_epochs(epochs, char)
        have = revealed.get(meta["player"], {})
        proven = all(e in have and have[e] <= t for e in rel)
        # `Kaleidoscope` draws from the other characters' pools, so their epochs count too.
        proven_all = all(e in have and have[e] <= t for e in epochs)
        rules["runs with every relevant epoch's content seen by then" if proven else "runs with unproven unlocks"] += 1
        every = all(have.get(PLAYED + c, t + 1) <= t for c in LOCKABLE_CHARACTERS)
        rules["runs with every character proven by a solo run" if every else "runs with a character unproven"] += 1
        raw = json.dumps(run)
        neow = [c["TextKey"] for c in run["map_point_history"][0][0]["player_stats"][0].get("ancient_choice", [])
                if c.get("was_chosen")] if run["map_point_history"] and run["map_point_history"][0] else []
        ctx = {"unlocks_proven": proven, "unlocks_proven_all": proven_all, "first_run": k in first_runs, "player": meta["player"],
               "all_characters": every,
               "modded": "BaseLib" in raw or re.search(r'"(?:CARD|RELIC|EVENT|ENCOUNTER|POTION)\.[A-Z0-9_]*-', raw),
               "relics": {bare(r["id"]) for r in run["players"][0]["relics"]},
               "last_floor": sum(len(a) for a in run["map_point_history"]),
               "died_in_fight": run.get("killed_by_encounter", "NONE.NONE") != "NONE.NONE",
               "neow": neow[0] if neow else None, "seed": run["seed"],
               "first_point": (run["map_point_history"][0][:1] or [{}])[0].get("map_point_type"),
               "unmapped_pull": next((f for f, e in enumerate((e for a in run["map_point_history"] for e in a), 1)
                                      if hidden_pulls(e, titles)[1]), None),
               "card_ok": {c["floor"]: c["status"] == "match" for c in result.get("replay", {}).get("checks", [])
                           if c["field"] == "card_reward"}}
        rows = []
        fitted = {g["floor"] for g in result.get("replay", {}).get("gaps", []) if g["why"].startswith("resynced")}
        for check, status, detail in outcomes(run, result):
            if detail.get("resynced") and detail.get("floor") in fitted and check in ("card_reward", "potion_reward",
                                                                                    "shop_cards", "shop_relics"):
                continue  # matched by the resync search itself
            if status == "mismatch":
                cls, reason = classify(check, detail, ctx)
                classes[cls, check, reason] += 1
                if cls == "profile":
                    status = "excluded"
                elif cls == "engine" and len(repro[check, reason]) < 4:
                    repro[check, reason].append({"character": char, "act1": act1, "seed": run["seed"],
                                                 "ascension": run["ascension"], "floor": detail.get("floor"),
                                                 "engine": detail.get("predicted"), "file": detail.get("recorded")})
                rows.append({"check": check, "class": cls, "reason": reason, **{k: v for k, v in detail.items()
                                                                               if k != "status"}})
            # Checks after a resync are counted apart: the chain state there is a best guess.
            stats[char, act1, check, "r-" + status if detail.get("resynced") else status] += 1
            by_act[check, detail.get("act", 1), "r-" + status if detail.get("resynced") else status] += 1
        report.append({"source": meta["source"], "character": char, "act1": act1, "seed": run["seed"],
                       "unlocks_proven": proven, "mismatches": rows,
                       "gaps": result.get("replay", {}).get("gaps", [])})
        if args.run:
            print(f"{meta['source']} {char} {act1} {run['seed']} A{run['ascension']} player {meta['player']}")
            for g in result.get("replay", {}).get("gaps", []):
                print(f"  gap f{g['floor']}: {g['why']}")
            for check, status, detail in outcomes(run, result):
                d = {k: v for k, v in detail.items() if k not in ("status", "field")}
                print(f"  {status:8} {check:14} {json.dumps(d)}")
        elif args.verbose and rows:
            print(f"  {meta['source']} {char} {act1} {run['seed']}: {[(r['check'], r['class']) for r in rows][:4]}")

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, indent=1))
    summary = {"runs": len(runs), "rules": dict(rules),
               "pass": {f"{c} {a} {k}": {s: stats[c, a, k, s] for s in ("match", "mismatch", "excluded", "r-match", "r-mismatch")
                                         if stats[c, a, k, s]}
                        for c, a, k, _ in sorted({x[:3] + (0,) for x in stats})},
               "by_act": {f"{c} act{a}": {st: by_act[c, a, st] for st in ("match", "mismatch", "excluded", "r-match", "r-mismatch")
                                          if by_act[c, a, st]}
                          for c, a, _ in sorted({x[:2] + (0,) for x in by_act})},
               "classes": [[cls, check, reason, n] for (cls, check, reason), n in classes.most_common()],
               "repro": {f"{k[0]}: {k[1]}": v for k, v in repro.items()}}
    args.summary.write_text(json.dumps(summary, indent=1))

    # Pass rates per check x character x act (excluded = classified profile state).
    print(f"{'check':15}" + "".join(f"{c[:4]}/{a[:2]:>2}".rjust(13) for c in CHARACTERS for a in ACTS))
    for check in CHECKS + ["error"]:
        cells = []
        for c in CHARACTERS:
            for a in ACTS:
                m, x = stats[c, a, check, "match"], stats[c, a, check, "mismatch"]
                cells.append(f"{m}/{m + x}".rjust(13) if m + x else "-".rjust(13))
        if any(cell.strip() != "-" for cell in cells):
            print(f"{check:15}" + "".join(cells))
    print("per act (match/checked, profile-excluded and after-resync checks not counted):")
    print(f"{'check':15}" + "".join(f"act {a}".rjust(13) for a in (1, 2, 3)))
    for check in CHECKS:
        cells = []
        for a in (1, 2, 3):
            m, x = by_act[check, a, "match"], by_act[check, a, "mismatch"]
            cells.append(f"{m}/{m + x}".rjust(13) if m + x else "-".rjust(13))
        if any(cell.strip() != "-" for cell in cells):
            print(f"{check:15}" + "".join(cells))
    rm = sum(n for (_, _, _, st), n in stats.items() if st == "r-match")
    rx = sum(n for (_, _, _, st), n in stats.items() if st == "r-mismatch")
    print(f"after a resync (not in the table): {rm}/{rm + rx} match")
    print("rules:", dict(rules))
    print("mismatch classes:")
    for (cls, check, reason), n in classes.most_common(40):
        print(f"  {n:6d} {cls:8} {check:15} {reason}")
    print(f"summary: {args.summary}; details: {args.out}")
    total = sum(n for (_, _, _, st), n in stats.items() if st in ("match", "mismatch"))
    bad = sum(n for (_, _, _, st), n in stats.items() if st == "mismatch")
    engine = sum(n for (cls, _, _), n in classes.items() if cls == "engine")
    unexplained = sum(n for (cls, _, r), n in classes.items() if cls == "engine" and not r.startswith("gap"))
    print(f"history-replay: {len(runs)} runs, {total - bad}/{total} checks match, "
          f"{sum(n for (_, _, _, st), n in stats.items() if st == 'excluded')} excluded (profile), "
          f"{engine} engine mismatches ({unexplained} not explained by an unported relic or map)")
    return 1 if unexplained else 0


if __name__ == "__main__":
    sys.exit(main())
