"""RLN41: run-level decision records from strong human Necrobinder A10 runs (the community dump).

    python python/rln41_dataset.py extract    # dump -> ~/.cache/spirebot-data/rln41/runs.jsonl.gz
    python python/rln41_dataset.py build      # runs -> decisions.pkl.gz, prints counts and drop rates

The dump is third-party with no license: everything read or written here stays under
`~/.cache/spirebot-data/rln41/` (`DATA`); nothing derived from it is committed.

**Players:** solo, unmodified, not abandoned A10 Necrobinder runs of players with >= 10 such runs and a
win rate > 50% (37 players, 1,409 runs). `HELDOUT` players (6, fixed) are the validation split.

**A record** (`dict`): `run`, `player`, `win`, `build`, `act` (0-2), `floor` (the file's count: the
first entry, Neow, is 1), `kind` (`KINDS`), `hp`, `max_hp`, `gold` (before the choice), `deck`
[(card vocab id, upgrade)], `relics` [relic vocab id], `options` [(token type, id, upgrade)] and
`choice` (index into `options`). Ids are the engine's vocabulary ids (`crates/sts2-rl/vocab/`, the line
number of the bare id, `CARD.X` -> `X`), so the same record can be built from the env's run obs
(`rln41_prior.obs_record`).

**Decision kinds** (one record per decision):
- `card_reward`: a combat room's card reward, options its cards + SKIP. A room with 6 or 9 offered
  cards is 2 or 3 rewards of 3 (file order); none picked in a reward is a skip.
- `rest`: HEAL, SMITH, OTHER (OTHER only when a relic or card offers one, or it was chosen); the first
  of `rest_site_choices`. HEAL is absent with `COFFEE_DRIPPER`, SMITH with `FUSION_HAMMER`.
- `smith_target`: after SMITH with one `upgraded_cards` entry, options the deck's upgradable cards
  (upgrade 0, not a curse or status), deduplicated by (card, upgrade).
- `shop_removal`: whether a shop visit used the removal (REMOVE or NONE; any `cards_removed` there).
- `shop_remove`: a shop's removal (`cards_removed`, one card), options the deck's removable cards.
- `shop_card`: a shop's cards are the file's `card_choices` (those left unbought) plus the cards
  gained there; one record per bought card (options the cards still for sale + NONE), then a NONE
  record. Prices are not in the file and are not used.
- `shop_relic`: the same over the shop's `relic_choices`.
- `ancient`: the ancient's relic options (`ancient_choice`).
- `event`: an event option (`event_choices` title key without `.title`; the env's text key). The
  file lists only the chosen option, so the options are every key seen on that page over the dataset
  (an approximation: options locked in this run may be listed). Pages with one known option are
  dropped.

**State before the choice:** the deck is rebuilt from the starting deck (`START_DECK`) plus each
earlier entry's gains, removals, transforms, upgrades and downgrades (checked against the final deck:
`rebuild_check`). Relics held are the final relics with `floor_added_to_deck` below the entry's floor
(the start relic always), plus `relics_removed` until their floor. HP is recorded after the room:
in a combat room the reward sees the post-fight HP (`current_hp`); at a rest site and a shop the HP
before is `current_hp - hp_healed`; at an event `current_hp - hp_healed + damage_taken`. Gold before
a shop is `current_gold + gold_spent`.

**Unknown ids:** a decision whose choice or an option has an id outside the vocabulary is dropped
(`drops`, per build); unknown deck cards or relics are kept as `<unk>` (1) and counted.
"""

import argparse
import collections
import gzip
import json
import os
import pickle
import random
import re
import sqlite3
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DATA = Path(os.path.expanduser("~/.cache/spirebot-data/rln41"))
DB = Path(os.path.expanduser("~/.cache/spirebot-data/sts2fun/sts2_public.db"))
RUNS = DATA / "runs.jsonl.gz"
DECISIONS = DATA / "decisions.pkl.gz"
VOCAB = ROOT / "crates/sts2-rl/vocab"
CARD_FILES = ROOT / "crates/sts2-core/src/content/cards"

FILTER = ("character='NECROBINDER' and ascension=10 and n_players=1 and has_modifiers=0 "
          "and was_abandoned=0")
N_HELDOUT = 6
HELDOUT_SEED = 41

START_DECK = ["STRIKE_NECROBINDER"] * 4 + ["DEFEND_NECROBINDER"] * 4 + ["BODYGUARD", "UNLEASH",
                                                                        "ASCENDERS_BANE"]
START_RELIC = "BOUND_PHYLACTERY"

KINDS = ["card_reward", "rest", "smith_target", "shop_removal", "shop_remove", "shop_card",
         "shop_relic", "ancient", "event"]
# Option token types.
T_CARD, T_SKIP, T_HEAL, T_SMITH, T_OTHER, T_RELIC, T_EVENT, T_NONE, T_REMOVE = range(1, 10)
REST_TOKENS = {"HEAL": T_HEAL, "SMITH": T_SMITH}
OTHER_REST_RELICS = {"SHOVEL", "GIRYA", "PAELS_GROWTH", "PUMPKIN_CANDLE", "MEAT_CLEAVER"}
OTHER_REST_CARDS = {"BYRDONIS_EGG"}
COMBAT_ROOMS = {"monster", "elite", "boss"}


def bare(model_id):
    return model_id.split(".", 1)[-1] if model_id else model_id


# ---------------------------------------------------------------------------------------------
# Vocabularies and card facts (read from the repo at run time)


class Vocab:
    """`crates/sts2-rl/vocab/<name>.txt`: id -> line number (1, `<unk>`, when absent)."""

    def __init__(self, name, root=VOCAB):
        self.ids = (root / f"{name}.txt").read_text().splitlines()
        self.num = {s: i for i, s in enumerate(self.ids) if i >= 2}

    def __call__(self, ident):
        return self.num.get(ident, 1)

    def __len__(self):
        return len(self.ids)

    def name(self, i):
        return self.ids[i] if 0 <= i < len(self.ids) else f"?{i}"


_CACHE = {}


def cards():
    if "cards" not in _CACHE:
        _CACHE["cards"] = Vocab("cards")
    return _CACHE["cards"]


def relics():
    if "relics" not in _CACHE:
        _CACHE["relics"] = Vocab("relics")
    return _CACHE["relics"]


def card_facts():
    """{CARD_ID: (type, eternal)} from the engine's card files (`CardType::X`, `CardKeyword::Eternal`)."""
    if "facts" not in _CACHE:
        out = {}
        for f in CARD_FILES.glob("*.rs"):
            if f.stem == "mod":
                continue
            text = f.read_text()
            m = re.search(r"CardType::(\w+)", text)
            out[f.stem.upper()] = (m.group(1) if m else "", "CardKeyword::Eternal" in text)
        _CACHE["facts"] = out
    return _CACHE["facts"]


def upgradable(card, upgrade):
    t = card_facts().get(card, ("", False))[0]
    return upgrade == 0 and t not in ("Curse", "Status")


def removable(card):
    return not card_facts().get(card, ("", False))[1] and card != "ASCENDERS_BANE"


def text_key_id(key):
    """`sts2_rl::run_obs::text_key_id`: FNV-1a of the key folded to 16 bits, never 0."""
    h = 0xCBF29CE484222325
    for b in key.encode():
        h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    f = h ^ (h >> 16) ^ (h >> 32) ^ (h >> 48)
    return max(f & 0xFFFF, 1)


# ---------------------------------------------------------------------------------------------
# Source


def players(db):
    q = (f"select player_id, count(*), sum(win) from runs where {FILTER} group by player_id "
         "having count(*) >= 10 and sum(win) * 1.0 / count(*) > 0.5 order by player_id")
    return db.execute(q).fetchall()


def heldout_players(ids):
    rng = random.Random(HELDOUT_SEED)
    return set(rng.sample(sorted(ids), N_HELDOUT))


def extract():
    DATA.mkdir(parents=True, exist_ok=True)
    db = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
    ids = [p for p, _, _ in players(db)]
    n = 0
    with gzip.open(RUNS, "wt") as f:
        q = (f"select id, player_id, win, build_id, raw_json from runs where {FILTER} and player_id in "
             f"({','.join(map(str, ids))}) order by id")
        for rid, pid, win, build, raw in db.execute(q):
            f.write(json.dumps({"id": rid, "player": pid, "win": int(win), "build": build,
                                "run": json.loads(raw)}) + "\n")
            n += 1
    print(f"{len(ids)} players, {n} runs -> {RUNS}")


def load_runs(path=RUNS):
    with gzip.open(path, "rt") as f:
        return [json.loads(line) for line in f]


# ---------------------------------------------------------------------------------------------
# Deck rebuild


def card_of(entry):
    """(bare id, upgrade) of a file card entry."""
    return bare(entry["id"]), int(entry.get("current_upgrade_level", 0) or 0)


class Deck:
    """A multiset of (card, upgrade) in insertion order."""

    def __init__(self, start=START_DECK):
        self.cards = [(c, 0) for c in start]
        self.misses = 0  # removals/upgrades of a card not in the deck

    def add(self, card):
        self.cards.append(card)

    def remove(self, card_id, upgrade=None):
        """Removes one copy, the given upgrade first, else any."""
        for want in ((upgrade,), None):
            for i, (c, u) in enumerate(self.cards):
                if c == card_id and (want is None or u in want):
                    return self.cards.pop(i)
        self.misses += 1
        return None

    def upgrade(self, card_id, delta=1):
        """Upgrades (or downgrades) the least (most) upgraded copy."""
        idx = [i for i, (c, _) in enumerate(self.cards) if c == card_id]
        if not idx:
            self.misses += 1
            return
        i = (min if delta > 0 else max)(idx, key=lambda j: self.cards[j][1])
        c, u = self.cards[i]
        self.cards[i] = (c, max(0, u + delta))

    def apply(self, stats):
        """An entry's deck changes, in a fixed order: removals, transforms, gains, upgrades."""
        for c in stats.get("cards_removed", []):
            self.remove(*card_of(c))
        for t in stats.get("cards_transformed", []):
            self.remove(*card_of(t["original_card"]))
            self.add(card_of(t["final_card"]))
        for c in stats.get("cards_gained", []):
            self.add(card_of(c))
        for c in stats.get("upgraded_cards", []):
            self.upgrade(bare(c), 1)
        for c in stats.get("downgraded_cards", []):
            self.upgrade(bare(c), -1)

    def counter(self):
        return collections.Counter(self.cards)


def final_deck(run):
    return collections.Counter(card_of(c) for c in run["players"][0]["deck"])


def rebuild_check(run):
    """(ids match, ids and upgrades match, misses) of the rebuilt deck against the final deck."""
    deck = Deck()
    for act in run["map_point_history"]:
        for e in act:
            for s in e["player_stats"][:1]:
                deck.apply(s)
    built, final = deck.counter(), final_deck(run)
    ids = collections.Counter(c for c, _ in deck.cards) == collections.Counter(c for c, _ in final.elements())
    return ids, built == final, deck.misses


# ---------------------------------------------------------------------------------------------
# HP and gold before the choice


def hp_before(stats, point_type, room_type):
    """HP the decision sees: the file's HP is after the room."""
    hp, healed, dmg = stats["current_hp"], stats.get("hp_healed", 0), stats.get("damage_taken", 0)
    if room_type in COMBAT_ROOMS or point_type in ("ancient", "treasure"):
        return hp
    if point_type in ("rest_site", "shop"):
        return max(1, hp - healed)
    return max(1, min(stats["max_hp"], hp - healed + dmg))


def gold_before(stats, point_type):
    g = stats.get("current_gold", 0)
    return g + stats.get("gold_spent", 0) if point_type == "shop" else g


# ---------------------------------------------------------------------------------------------
# Relics held


def relic_timeline(run):
    """[(relic, from floor, until floor or None)]: held at floor f if from < f (start relic always)."""
    out = []
    for r in run["players"][0]["relics"]:
        rid = bare(r["id"])
        start = 0 if rid == START_RELIC else int(r.get("floor_added_to_deck", 1))
        out.append((rid, start, None))
    floor = 0
    for act in run["map_point_history"]:
        for e in act:
            floor += 1
            for s in e["player_stats"][:1]:
                for r in s.get("relics_removed", []):
                    out.append((bare(r), 0, floor))
    return out


def relics_at(timeline, floor):
    return [r for r, a, b in timeline if a < floor and (b is None or floor <= b)]


# ---------------------------------------------------------------------------------------------
# Decisions


def card_groups(choices):
    """A room's offered cards split into rewards of 3 (6 or 9 cards), else one reward."""
    n = len(choices)
    if n > 5 and n % 3 == 0:
        return [choices[i:i + 3] for i in range(0, n, 3)]
    return [choices]


def event_key(choice):
    t = (choice.get("title") or {})
    if t.get("table") != "events":
        return None
    k = t.get("key", "")
    return k[:-len(".title")] if k.endswith(".title") else None


def page_of(key):
    return key.rsplit(".options.", 1)[0] if ".options." in key else None


def event_catalog(runs):
    """{page: sorted option keys seen} over all runs."""
    pages = collections.defaultdict(set)
    for x in runs:
        for act in x["run"]["map_point_history"]:
            for e in act:
                for s in e["player_stats"][:1]:
                    for c in s.get("event_choices", []):
                        k = event_key(c)
                        if k and page_of(k):
                            pages[page_of(k)].add(k)
    return {p: sorted(v) for p, v in pages.items()}


class Builder:
    """Turns runs into records, counting drops per build and kind."""

    def __init__(self, catalog=None):
        self.cv, self.rv = cards(), relics()
        self.catalog = catalog or {}
        self.records = []
        self.drops = collections.Counter()   # (build, kind) -> dropped
        self.kept = collections.Counter()    # (build, kind) -> kept
        self.unk_deck = 0
        self.unk_relic = 0
        self.deck_cards = 0
        self.relic_slots = 0

    def card_tok(self, card):
        c, u = card
        return (T_CARD, self.cv(c), u)

    def emit(self, meta, kind, state, options, choice):
        build = meta["build"]
        if any(t in (T_CARD, T_RELIC) and i == 1 for t, i, _ in options):
            self.drops[(build, kind)] += 1
            return
        self.kept[(build, kind)] += 1
        self.records.append(dict(meta, kind=kind, options=options, choice=choice, **state))

    def run(self, x):
        run = x["run"]
        meta = dict(run=x["id"], player=x["player"], win=x["win"], build=x["build"])
        deck = Deck()
        timeline = relic_timeline(run)
        floor = 0
        for act_i, act in enumerate(run["map_point_history"]):
            for e in act:
                floor += 1
                stats = e["player_stats"][0] if e["player_stats"] else None
                if stats is None:
                    continue
                pt = e["map_point_type"]
                rooms = e.get("rooms") or [{}]
                room_type = rooms[-1].get("room_type", "")
                held = relics_at(timeline, floor)
                deck_ids = [(self.cv(c), u) for c, u in deck.cards]
                rel_ids = [self.rv(r) for r in held]
                self.deck_cards += len(deck_ids)
                self.unk_deck += sum(i == 1 for i, _ in deck_ids)
                self.relic_slots += len(rel_ids)
                self.unk_relic += sum(i == 1 for i in rel_ids)
                state = dict(act=act_i, floor=floor, hp=hp_before(stats, pt, room_type),
                             max_hp=stats["max_hp"], gold=gold_before(stats, pt),
                             deck=deck_ids, relics=rel_ids)
                self.entry(meta, state, stats, pt, room_type, deck, held)
                deck.apply(stats)

    def entry(self, meta, state, s, pt, room_type, deck, held):
        cv, rv = self.cv, self.rv
        # Card rewards
        if room_type in COMBAT_ROOMS and s.get("card_choices"):
            for group in card_groups(s["card_choices"]):
                opts = [self.card_tok(card_of(c["card"])) for c in group] + [(T_SKIP, 0, 0)]
                picked = [i for i, c in enumerate(group) if c.get("was_picked")]
                if len(picked) > 1:
                    self.drops[(meta["build"], "card_reward")] += 1
                    continue
                self.emit(meta, "card_reward", state, opts, picked[0] if picked else len(group))
        # Rest site
        if pt == "rest_site" and s.get("rest_site_choices"):
            ch = s["rest_site_choices"][0]
            opts = []
            if "COFFEE_DRIPPER" not in held or ch == "HEAL":
                opts.append((T_HEAL, 0, 0))
            if "FUSION_HAMMER" not in held or ch == "SMITH":
                opts.append((T_SMITH, 0, 0))
            other = (ch not in REST_TOKENS or OTHER_REST_RELICS & set(held)
                     or any(c in OTHER_REST_CARDS for c, _ in deck.cards))
            if other:
                opts.append((T_OTHER, 0, 0))
            tok = REST_TOKENS.get(ch, T_OTHER)
            self.emit(meta, "rest", state, opts, [t for t, _, _ in opts].index(tok))
            ups = s.get("upgraded_cards", [])
            if ch == "SMITH" and len(ups) == 1:
                target = bare(ups[0])
                cand = sorted({(c, u) for c, u in deck.cards if upgradable(c, u)})
                if (target, 0) in cand and len(cand) > 1:
                    opts = [self.card_tok(c) for c in cand]
                    self.emit(meta, "smith_target", state, opts, cand.index((target, 0)))
                else:
                    self.drops[(meta["build"], "smith_target")] += 1
        if pt == "shop":
            self.shop(meta, state, s, deck)
        # Ancient
        if s.get("ancient_choice"):
            anc = s["ancient_choice"]
            chosen = [i for i, a in enumerate(anc) if a.get("was_chosen")]
            if len(anc) > 1 and len(chosen) == 1:
                opts = [(T_RELIC, rv(a["TextKey"]), 0) for a in anc]
                self.emit(meta, "ancient", state, opts, chosen[0])
        # Events
        if pt != "ancient":
            for c in s.get("event_choices", []):
                k = event_key(c)
                page = page_of(k) if k else None
                keys = self.catalog.get(page, [])
                if k is None or len(keys) < 2:
                    continue
                opts = [(T_EVENT, text_key_id(o), 0) for o in keys]
                self.emit(meta, "event", state, opts, keys.index(k))

    def shop(self, meta, state, s, deck):
        cv, rv = self.cv, self.rv
        removed = s.get("cards_removed", [])
        self.emit(meta, "shop_removal", state, [(T_REMOVE, 0, 0), (T_NONE, 0, 0)], 0 if removed else 1)
        if len(removed) == 1:
            target = card_of(removed[0])
            cand = sorted({(c, u) for c, u in deck.cards if removable(c)})
            if target in cand and len(cand) > 1:
                self.emit(meta, "shop_remove", state, [self.card_tok(c) for c in cand],
                          cand.index(target))
            else:
                self.drops[(meta["build"], "shop_remove")] += 1
        # Cards: the file lists the unbought ones; bought ones are gains here.
        unbought = [card_of(c["card"]) for c in s.get("card_choices", [])]
        bought = [card_of(c) for c in s.get("cards_gained", [])]
        if unbought or bought:
            self.sequence(meta, "shop_card", state, [self.card_tok(c) for c in unbought],
                          [self.card_tok(c) for c in bought])
        rel = s.get("relic_choices", [])
        if rel:
            self.sequence(meta, "shop_relic", state,
                          [(T_RELIC, rv(bare(r["choice"])), 0) for r in rel if not r.get("was_picked")],
                          [(T_RELIC, rv(bare(r["choice"])), 0) for r in rel if r.get("was_picked")])

    def sequence(self, meta, kind, state, unbought, bought):
        """One record per bought item (options: everything still for sale + NONE), then NONE."""
        left = list(unbought) + list(bought)
        for b in bought:
            opts = left + [(T_NONE, 0, 0)]
            self.emit(meta, kind, state, opts, opts.index(b))
            left.remove(b)
        if left:
            self.emit(meta, kind, state, left + [(T_NONE, 0, 0)], len(left))


def build(runs, heldout):
    b = Builder(event_catalog(runs))
    for x in runs:
        b.run(x)
    for r in b.records:
        r["heldout"] = r["player"] in heldout
    return b


def summary(b, runs):
    builds = sorted({x["build"] for x in runs}, key=lambda v: tuple(int(p) for p in v[1:].split(".")))
    checks = [rebuild_check(x["run"]) for x in runs]
    print(f"deck rebuild vs final deck: ids {sum(c[0] for c in checks)}/{len(runs)}, "
          f"ids+upgrades {sum(c[1] for c in checks)}/{len(runs)}, runs with a miss "
          f"{sum(c[2] > 0 for c in checks)}")
    print(f"unknown deck cards {b.unk_deck}/{b.deck_cards}, unknown relics {b.unk_relic}/{b.relic_slots}")
    kinds = collections.Counter(r["kind"] for r in b.records)
    print("records:", dict(kinds), "total", len(b.records))
    print("heldout records:", sum(r["heldout"] for r in b.records))
    print("build   kept  dropped  drop%")
    for v in builds:
        k = sum(n for (bb, _), n in b.kept.items() if bb == v)
        d = sum(n for (bb, _), n in b.drops.items() if bb == v)
        print(f"{v:8} {k:6} {d:6} {100 * d / max(1, k + d):6.2f}")
    print("kind    kept  dropped  drop%")
    for kd in KINDS:
        k = sum(n for (_, kk), n in b.kept.items() if kk == kd)
        d = sum(n for (_, kk), n in b.drops.items() if kk == kd)
        print(f"{kd:12} {k:6} {d:6} {100 * d / max(1, k + d):6.2f}")


def main(argv=None):
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["extract", "build"])
    args = ap.parse_args(argv)
    if args.cmd == "extract":
        extract()
        return
    runs = load_runs()
    heldout = heldout_players({x["player"] for x in runs})
    b = build(runs, heldout)
    summary(b, runs)
    with gzip.open(DECISIONS, "wb") as f:
        pickle.dump(dict(records=b.records, heldout=sorted(heldout), kept=dict(b.kept),
                         drops=dict(b.drops)), f)
    print(f"-> {DECISIONS}")


if __name__ == "__main__":
    sys.exit(main())
