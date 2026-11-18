"""Run-env bytes -> entity tokens, on the GPU (`docs/plans/necro-e2e.md`, WP3).

`encode(obs, mask)` turns a batch of `Sts2RunEnvs.obs` rows (uint8, n x 2,996, layout
`crates/sts2-rl/src/run_obs.rs`) and their masks into a fixed-width token batch, all with
integer gathers and elementwise maths (no Python loop over rows, no float reduction), so a row's
tokens are the same bits in any batch. `encode_priv(priv)` does the same for the privileged
buffer (`crates/sts2-rl/src/priv_obs.rs`), which only the value module reads.

**Tokens.** One token per non-empty slot that matters in the current phase, `T_MAX` wide:
- fixed positions 0-4: the STATE token (run header, combat header, the pending choice's and
  selection's numbers; its `cat` is the phase) and four pooled pile summaries (draw, discard,
  exhaust in a fight; the deck outside one), each a mean of its cards' ID embeddings (`bag_*`,
  pooled in the net) plus counts;
- then, compacted in this order: bosses, the room's encounter or event, the ancient, hand
  cards, creatures (the player is creature 0, Osty and other allies next, then the enemies),
  their powers (one token each, linked to the creature by `owner`), intents, combat choice
  options, the belt, relics, the deck (outside fights, in phases where it is the choice's
  context), map points of the next `MAP_ROWS` rows (map phase), ancient and event options,
  rewards and their cards, deck-selection options, bundle cards, rest options, the chest, the
  shop's entries and its removal.

A token is `kind` (`KINDS`), `ident` (a row of the shared ID table: the `vocab` id of its card,
relic, monster, power, potion, encounter, event, ancient or act at a fixed offset per kind of
content, `ID_ROWS` rows in all), `cat` (a small categorical: phase, pile, reward kind, intent
kind, rest option, map point type...), `owner` (a link: creature slot + 1 for a creature and
its powers, reward + 1 for a reward and its cards, bundle + 1), `num` (`NUM` float columns,
symlog of the raw numbers; zero columns are inert) and `valid`. Tokens past `T_MAX` are dropped
and counted (`overflow`).

**Action entities.** For each of the 361 indices (`ACTIONS_ALL`), `ent` is the position of the token it acts on
(`sts2.action_table()`: hand slot -> hand token, belt slot, option, map column -> the legal
next point in that column, reward, reward card, shop slot...; end turn, done, confirm, cancel
and proceed act on the STATE token) and `tgt` the position of a play's or potion's target
creature (enemy `sub - 1` in order), else -1. An entity whose token was dropped falls back to
STATE.

**v2** (`encode(..., v2=True)`, rows with the `obs_v2` section, RLN16): extra per-token inputs
that a v1 net ignores and a v2 net reads through zero-initialised weights, so the tokens above
stay exactly the same:
- `link`: an intent's enemy (its creature slot + 1, the creature tokens' `owner`), 0 elsewhere;
- `xcat` (B x T x 3): a creature's forecast move kinds, step k at `k * FC_KINDS + kind`;
- `xnum` (B x T x `XNUM`): a creature's forecast (certainty, damage, hits, whole attack per
  step), this hand's knapsack max damage on it and whether that kills it; a hand card's damage
  and block previews, hits and whether it is playable; the STATE token's HP lost if the turn
  ended now; a map point's route summaries to the boss (`_route`);
  RLN46: the HP lost subtracts what Osty (a living ally with DieForYou) absorbs, its amount
  on the STATE token (col 5); a creature's col 15 and the STATE token's col 4 flag Doom kills;
- `znum` (B x T x `ZNUM`, RLN21, read by its own zero-initialised stem): a creature's facing
  (faces right, faces left) and, for a crab arm, whether it is behind the player (its attacks
  deal 1.5x) or in front; a power's shown number where its icon shows one other than the amount
  (1, symlog of it);
- `anum` (B x 355 or, with `discard`, 361 x `ANUM`, RLN22, read by the policy head's own zero-initialised map): per
  action index, the plan column (`plan_column.rs`, written by the env for the combat indices
  its mask holds): scored, the planner's choice, a winning play, symlog(score / 10) and
  symlog((score - the row's best score) / 10);
- `y_*` (B x `MAPNODES`): the reachable map points (the cone from the legal next points, in
  row then column order) as separate tokens, with the same route summaries.

**Full view** (`encode(..., v2=True, full=True)`, rows with the `obs_full` section, RLN30): what
a player sees that the tokens above leave out. `T_MAX_FULL` wide; with `full` off nothing here
exists and every tensor is the same as without it. Two more sections after the shop's (map rows,
then piles), so they are the first dropped when a row overflows:
- `pile` tokens (kind `FULL_KINDS`, after `KINDS`; RLN31): one per distinct (card, upgrade,
  cost, flags) among the draw, discard and exhaust piles' cards in a fight (the combat section's
  card table, `CARDS` = 64 slots), `num` as a hand card's (upgrade, cost, X cost, the six flags)
  then symlog of its copies in the draw, discard and exhaust piles, `cat` the piles
  it is in (bit 0 draw, 1 discard, 2 exhaust), and the content stats as for hand cards. They
  come sorted by the card's bytes, never in the table's or the pile's order: the draw pile's
  order is hidden.
- the rest of the act's map (`map_row` tokens, RLN31): one per map row ahead that holds a point
  the `MAP_ROWS` window leaves out, outside fights in every phase (the window's tokens are
  map-phase only), in row order. `num` the rows ahead and the row's point count; per column c
  (`MAP_COLS`) two `hcat` slots: at 2c the point's type (`MAP_TYPE_AT + c * MAP_TYPES + type`)
  and at 2c + 1 its edges (`MAP_EDGE_AT + c * 16 + bits`: a child at c - 1, c, c + 1, and one
  further away, which is the boss), 0 for no point. The route summaries stay on the window's
  points and the MAPNODE tokens.
- each creature's past moves (`sts2_rl::obs_full`: the engine's move log, newest first, at most
  `HIST`) on its creature token: `hcat` (B x T x `HCAT`, its first `HIST` slots; step k at `k * FC_KINDS + kind + 2`,
  `+ 1` when its intent was not seen, 0 none), `hmove` (B x T x `HIST`, the move's key:
  `1 + k * MOVE_BUCKETS + key % MOVE_BUCKETS`, 0 none) and `hnum` (B x T x `HNUM`: per step present, whether its intent was seen, damage per
  hit, hits, whole attack, as it showed; then how many past moves the log counts). The intent
  is the one the observer saw while the move was current; a move not seen (decided between two
  observations) has its key but kind and numbers 0.

**Event options** (`encode(..., v2=True, events=True)`, rows with the `obs_events` section,
RLN33): what each ancient and event option's text tells a player (`sts2_rl::obs_events`), per
row for the page's 8 options: `evec` (B x 8 x `EV_COLS`, symlog of the numbers: HP, max HP and
gold changes, cards added, removed, upgraded, transformed..., relics and potions gained and
lost, a fight, flags; column 0 is 1 when the option is fully described), `eids` (B x 8 x
`EV_IDS`, ID table rows of the card, curse, relic and potion it gives and the card, relic and
potion it takes, 0 none) and `ench` (B x 8, the enchantment it applies, `EnchantmentKind` index
+ 1, 0 none); `ev_slot` (B x T) is 1 + the option a token reads (ancient and event option
tokens), 0 elsewhere. Kept per row, not per token, so the rollout buffers stay small. With
`events` off nothing here exists.
"""

import numpy as np
import torch

from .. import env as _env

T_MAX = 128
"""Tokens per row (the encoder's width; the net trims a batch to its longest row)."""
T_MAX_FULL = 128
"""Tokens per row with the full view (RLN31: the widest row under NECRO2's policy is 102)."""
NUM = 48
"""Numeric columns per token."""
BAG = 80
"""Cards pooled into the summaries."""
MAP_ROWS = 4
"""Map rows ahead shown as tokens in the map phase."""
ACTIONS = 355
"""The width of `legal`, the logits and `anum` without `discard_potion`: the space before RLN32, so
the same shapes, kernels and bits as a net from before it."""
ACTIONS_ALL = 361
"""The whole run space (`run_action.rs`): the 355 above, then RLN32's discard-potion slots (listed
only with the `discard_potion` option). The action tables, `ent` and `tgt` always span it."""
FIXED = 5
"""Fixed positions: STATE and the four summaries."""

KINDS = (
    "state", "sum_draw", "sum_discard", "sum_exhaust", "sum_deck", "boss", "page", "ancient",
    "hand", "creature", "power", "intent", "choice", "belt", "relic", "deck", "map",
    "ancient_option", "event_option", "reward", "reward_card", "select", "bundle_card", "rest",
    "treasure", "shop_card", "shop_relic", "shop_potion", "shop_remove",
)
FULL_KINDS = ("pile", "map_row")
"""Token kinds of the full view (RLN30), numbered after `KINDS` (a net without it has no rows
for them)."""
K = {name: i for i, name in enumerate(KINDS + FULL_KINDS)}

# The shared ID table: each vocabulary at a fixed offset with room to grow (append-only files),
# then the event options' text-key hashes folded into buckets.
_OFF = {}
_at = 0
for _name, _cap in (("cards", 1024), ("relics", 512), ("monsters", 256), ("powers", 512),
                    ("potions", 128), ("encounters", 256), ("events", 128), ("ancients", 32),
                    ("acts", 16), ("text", 1024)):
    _OFF[_name] = _at
    _at += _cap
TEXT_BUCKETS = 1024
ID_ROWS = 4096
assert _at <= ID_ROWS

# Run observation offsets (`run_obs.rs`; checked against the library's layout in `check_layout`).
DECK_AT, RELICS_AT, BELT_AT, MAP_AT = 32, 352, 480, 492
ANCIENT_AT, EVENT_AT, REWARDS_AT, SELECT_AT = 972, 998, 1024, 1192
BUNDLES_AT, REST_AT, TREASURE_AT, SHOP_AT, COMBAT_AT = 1600, 1648, 1664, 1666, 1750
OBS_BYTES = 2996
"""The v1 bytes: all the encoder reads with `obs_v2` off (an ABI-8 library's rows are wider)."""
V2_AT = 2996
OBS_BYTES_V2 = 4116
FULL_AT = 4116
OBS_BYTES_FULL = 4340
"""The rows with the full-view section (`obs_full.rs`, ABI 12; `FULL_AT` on)."""
EVENTS_AT = 4340
OBS_BYTES_EVENTS = 4980
"""The rows with the event section (`obs_events.rs`, ABI 14; `EVENTS_AT` on)."""
EV_COLS, EV_IDS = 32, 7
"""Numbers and id slots per option (`obs_events::COLUMNS`, `ID_SLOTS`)."""
EV_OPT = 2 * EV_COLS + 2 * EV_IDS + 2
"""Bytes per option."""
EV_ID_TABLES = ("cards", "cards", "relics", "potions", "cards", "relics", "potions")
"""The vocabulary of each id slot (card, curse, relic, potion, lost card, relic, potion)."""
"""The rows with the v2 section (`obs_v2.rs`, ABI 10; ABI 9's were 3,696 bytes, the same
without RLN22's plan column at the end; ABI 8's 3,392, also without RLN21's facing and shown
numbers)."""
# Inside the combat section (`obs.rs`).
C_CREATURES, C_CARDS, C_HAND, C_INTENTS, C_CHOICE = 24, 504, 888, 908, 948
CREATURE_BYTES = 60
# Action blocks (`run_action.rs`, `action.rs`).
PICK_BASE = 129
ANCIENT_BASE = 140

PHASE_COMBAT, PHASE_MAP = 3, 2


def obs_width(v2, full=False, events=False):
    """The bytes per row the encoder (and the learner's buffers) take: rows are cut to it."""
    return (OBS_BYTES_EVENTS if events else OBS_BYTES_FULL if full else OBS_BYTES_V2 if v2
            else OBS_BYTES)


def t_max(full=False):
    """The encoder's token width."""
    return T_MAX_FULL if full else T_MAX


def check_layout(layout, v2=False, full=False, events=False):
    """Raises if the library's run layout is not the one this encoder reads (ABI 6-7 rows are
    `OBS_BYTES` wide, ABI 10-11 rows `OBS_BYTES_V2`, ABI 12-13 rows `OBS_BYTES_FULL`, ABI 14
    rows `OBS_BYTES_EVENTS`, each starting with the narrower ones; `v2` needs ABI 10 on, `full`
    ABI 12, `events` ABI 14; ABI 8-9 rows are neither: rebuild the library)."""
    rows = int(layout.obs_bytes)
    if (rows not in (OBS_BYTES, OBS_BYTES_V2, OBS_BYTES_FULL, OBS_BYTES_EVENTS)
            or (v2 and rows == OBS_BYTES) or (full and rows < OBS_BYTES_FULL)
            or (events and rows != OBS_BYTES_EVENTS)):
        raise ValueError(f"run obs of {rows} bytes, the encoder reads "
                         f"{obs_width(v2, full, events)} (v2={v2}, full={full}, events={events})")
    if rows != OBS_BYTES and int(layout.v2_at) != V2_AT:
        raise ValueError(f"v2 section at {int(layout.v2_at)}, the encoder reads {V2_AT}")
    if rows >= OBS_BYTES_FULL and int(layout.full_at) != FULL_AT:
        raise ValueError(f"full section at {int(layout.full_at)}, the encoder reads {FULL_AT}")
    if rows == OBS_BYTES_EVENTS and int(layout.events_at) != EVENTS_AT:
        raise ValueError(f"event section at {int(layout.events_at)}, "
                         f"the encoder reads {EVENTS_AT}")
    want = dict(deck_at=DECK_AT, relics_at=RELICS_AT, belt_at=BELT_AT,
                map_at=MAP_AT, ancient_at=ANCIENT_AT, event_at=EVENT_AT, rewards_at=REWARDS_AT,
                select_at=SELECT_AT, bundles_at=BUNDLES_AT, rest_at=REST_AT,
                treasure_at=TREASURE_AT, shop_at=SHOP_AT, combat_at=COMBAT_AT, actions=ACTIONS_ALL,
                priv_bytes=PRIV_BYTES)
    got = {k: int(getattr(layout, k)) for k in want}
    if got != want:
        raise ValueError(f"run layout {got}, the encoder reads {want}")


def symlog(x):
    x = x.float()
    return torch.sign(x) * torch.log1p(x.abs())


class _Bytes:
    """Little-endian field reads over an int32 copy of the rows."""

    def __init__(self, obs):
        self.o = obs.to(torch.int32)
        self.dev = obs.device

    def cols(self, base, count, stride, off):
        idx = base + off + stride * torch.arange(count, device=self.dev)
        return idx

    def u8(self, base, count=1, stride=1, off=0):
        return self.o[:, self.cols(base, count, stride, off)]

    def u16(self, base, count=1, stride=2, off=0):
        idx = self.cols(base, count, stride, off)
        return self.o[:, idx] | (self.o[:, idx + 1] << 8)

    def i16(self, base, count=1, stride=2, off=0):
        v = self.u16(base, count, stride, off)
        return v - ((v >= 32768).to(torch.int32) << 16)

    def i8(self, base, count=1, stride=1, off=0):
        v = self.u8(base, count, stride, off)
        return v - ((v >= 128).to(torch.int32) << 8)


def _ids(raw, table):
    """Vocab ids -> ID table rows (0 stays the table's pad row of that kind)."""
    return raw.long() + _OFF[table]


class _Section:
    """One block of candidate tokens: (B, n) fields."""

    def __init__(self, kind, valid, ident=None, cat=None, owner=None, num=()):
        b, n = valid.shape
        self.kind, self.valid, self.n = kind, valid, n
        z = torch.zeros(b, n, dtype=torch.long, device=valid.device)
        self.ident = z if ident is None else ident.long()
        self.cat = z if cat is None else cat.long()
        self.owner = z if owner is None else owner.long()
        self.num = list(num)
        # v2 extras (`_v2`), None: zeros.
        self.xnum, self.xcat, self.link, self.znum = (), None, None, ()
        # The full view's past moves (`_full_extras`, creatures only), None: zeros.
        self.full = None
        # Which tokens read the event section (`_event_extras`; token k is option k), None: none.
        self.ev = None


def _numcat(cols, b, n, dev):
    out = torch.zeros(b, n, NUM, device=dev)
    for i, c in enumerate(cols):
        out[:, :, i] = c.float() if c.dim() == 2 else c.float().unsqueeze(1)
    return out


def _sections(r, mask_bits, full=False):
    """Every candidate section of a batch, in token order (after the fixed positions); `full`
    appends the full view's (module doc)."""
    b = r.o.shape[0]
    dev = r.dev
    phase = r.u8(3)[:, 0]
    combat = phase == PHASE_COMBAT
    out = []

    def ph(*codes):
        m = torch.zeros_like(phase, dtype=torch.bool)
        for c in codes:
            m |= phase == c
        return m.unsqueeze(1)

    # Bosses.
    boss = r.u16(20, 2)
    out.append(_Section(K["boss"], boss > 0, _ids(boss, "encounters"),
                        cat=torch.arange(2, device=dev).expand(b, 2)))
    # The room's encounter (combat room) or event.
    room_type = r.u8(26)
    model = r.u16(28)
    page_id = torch.where(room_type > 0, _ids(model, "encounters"), _ids(model, "events"))
    out.append(_Section(K["page"], model > 0, page_id, cat=room_type))
    # The ancient.
    anc = r.u16(ANCIENT_AT)
    out.append(_Section(K["ancient"], (anc > 0) & ph(0), _ids(anc, "ancients")))

    # Fight: hand, creatures, powers, intents, choice options.
    cards_kind = r.u16(COMBAT_AT + C_CARDS, 64, 6)
    cards_up = r.u8(COMBAT_AT + C_CARDS, 64, 6, 2)
    cards_pile = r.u8(COMBAT_AT + C_CARDS, 64, 6, 3)
    cards_cost = r.i8(COMBAT_AT + C_CARDS, 64, 6, 4)
    cards_flags = r.u8(COMBAT_AT + C_CARDS, 64, 6, 5)

    def card_of(card_ids, valid):
        idx = card_ids.long().clamp(0, 63)
        g = lambda t: torch.gather(t, 1, idx)  # noqa: E731
        ok = valid & (card_ids < 64)
        kind, up, cost, flags = g(cards_kind), g(cards_up), g(cards_cost), g(cards_flags)
        bits = [((flags >> k) & 1) for k in range(6)]
        num = [up, cost, (cost < 0).float()] + bits
        return ok, _ids(kind, "cards"), num

    hand_size = r.u8(COMBAT_AT + 16)[:, 0:1]
    hand_ids = r.u16(COMBAT_AT + C_HAND, 10)
    slots = torch.arange(10, device=dev).expand(b, 10)
    ok, ident, num = card_of(hand_ids, (slots < hand_size) & combat.unsqueeze(1))
    out.append(_Section(K["hand"], ok, ident, num=num + [slots]))

    cb = COMBAT_AT + C_CREATURES
    alive = r.u8(cb, 8, CREATURE_BYTES, 0)
    side = r.u8(cb, 8, CREATURE_BYTES, 1)
    monster = r.u16(cb, 8, CREATURE_BYTES, 4)
    hp = r.i16(cb, 8, CREATURE_BYTES, 6)
    max_hp = r.i16(cb, 8, CREATURE_BYTES, 8)
    block = r.i16(cb, 8, CREATURE_BYTES, 10)
    creature_ok = (max_hp > 0) & combat.unsqueeze(1)
    cslot = torch.arange(8, device=dev).expand(b, 8)
    frac = hp.float() / max_hp.clamp(min=1).float()
    out.append(_Section(K["creature"], creature_ok, _ids(monster, "monsters"),
                        cat=side * 2 + (cslot == 0).long() * (side == 0).long(),
                        owner=cslot + 1,
                        num=[alive, symlog(hp), symlog(max_hp), symlog(block), frac]))
    # Power j of creature c sits at cb + 60c + 12 + 4j.
    idx = (cb + 12 + CREATURE_BYTES * torch.arange(8, device=dev).unsqueeze(1)
           + 4 * torch.arange(12, device=dev).unsqueeze(0)).flatten()
    pk = (r.o[:, idx] | (r.o[:, idx + 1] << 8)).view(b, 96)
    pa = (r.o[:, idx + 2] | (r.o[:, idx + 3] << 8))
    pa = (pa - ((pa >= 32768).to(torch.int32) << 16)).view(b, 96)
    powner = torch.arange(8, device=dev).repeat_interleave(12).expand(b, 96)
    power_ok = (pk > 0) & torch.gather(creature_ok, 1, powner)
    out.append(_Section(K["power"], power_ok, _ids(pk, "powers"), owner=powner + 1,
                        num=[symlog(pa), (pa < 0).float()]))

    ik = r.u8(COMBAT_AT + C_INTENTS, 8, 5, 0)
    idmg = r.i16(COMBAT_AT + C_INTENTS, 8, 5, 1)
    ihits = r.u8(COMBAT_AT + C_INTENTS, 8, 5, 3)
    istat = r.u8(COMBAT_AT + C_INTENTS, 8, 5, 4)
    out.append(_Section(K["intent"], (ik > 0) & combat.unsqueeze(1), cat=ik,
                        num=[symlog(idmg), ihits, symlog(idmg * ihits), istat,
                             torch.arange(8, device=dev).expand(b, 8)]))

    choice_ids = r.u16(COMBAT_AT + C_CHOICE, 10)
    choice_live = mask_bits[:, PICK_BASE:PICK_BASE + 10] & combat.unsqueeze(1)
    ok, ident, num = card_of(choice_ids, choice_live)
    out.append(_Section(K["choice"], ok, ident, num=num))

    # Belt (live in a fight), relics.
    belt = r.u16(BELT_AT, 6)
    out.append(_Section(K["belt"], belt > 0, _ids(belt, "potions"),
                        num=[torch.arange(6, device=dev).expand(b, 6)]))
    relic = r.u16(RELICS_AT, 32, 4)
    rflags = r.u8(RELICS_AT, 32, 4, 2)
    out.append(_Section(K["relic"], relic > 0, _ids(relic, "relics"),
                        num=[rflags & 1, (rflags >> 1) & 1]))

    # The deck, where it is the context of the choice.
    deck = r.u16(DECK_AT, 80, 4)
    dup = r.u8(DECK_AT, 80, 4, 2)
    dfl = r.u8(DECK_AT, 80, 4, 3)
    out.append(_Section(K["deck"], (deck > 0) & ph(0, 1, 4, 6, 7, 9), _ids(deck, "cards"),
                        cat=dfl >> 1, num=[dup, dfl & 1]))

    # Map window.
    col = r.u8(MAP_AT, 96, 5, 0)
    row = r.u8(MAP_AT, 96, 5, 1)
    ptype = r.u8(MAP_AT, 96, 5, 2)
    pflags = r.u8(MAP_AT, 96, 5, 3)
    child = r.u8(MAP_AT, 96, 5, 4)
    cur_row = r.u8(25)
    cur = torch.where(cur_row == 255, torch.full_like(cur_row, -1), cur_row)
    ahead = row - cur
    map_ok = (ptype > 0) & (ahead > 0) & (ahead <= MAP_ROWS) & ph(PHASE_MAP)
    cur_col = r.u8(24).float()
    out.append(_Section(K["map"], map_ok, cat=ptype,
                        num=[col, ahead, col.float() - cur_col, (pflags >> 2) & 1]
                        + [((child >> k) & 1) for k in range(7)]))

    # Ancient and event options.
    arel = r.u16(ANCIENT_AT + 2, 8, 3)
    alock = r.u8(ANCIENT_AT + 2, 8, 3, 2)
    a_ok = ((arel > 0) | mask_bits[:, ANCIENT_BASE:ANCIENT_BASE + 8]) & ph(0)
    out.append(_Section(K["ancient_option"], a_ok, _ids(arel, "relics"),
                        num=[alock, torch.arange(8, device=dev).expand(b, 8)]))
    ekey = r.u16(EVENT_AT + 2, 8, 3)
    een = r.u8(EVENT_AT + 2, 8, 3, 2)
    out.append(_Section(K["event_option"], (ekey > 0) & ph(1),
                        ekey.long() % TEXT_BUCKETS + _OFF["text"],
                        num=[een, torch.arange(8, device=dev).expand(b, 8)]))

    # Rewards and their cards.
    rk = r.u8(REWARDS_AT, 8, 21, 0)
    rfl = r.u8(REWARDS_AT, 8, 21, 1)
    rv = r.u16(REWARDS_AT, 8, 21, 2)
    rup = r.u8(REWARDS_AT, 8, 21, 4)
    rid = torch.where(rk == 2, _ids(rv, "potions"),
                      torch.where((rk == 3) | (rk == 4), _ids(rv, "relics"),
                                  torch.where(rk == 6, _ids(rv, "cards"),
                                              torch.zeros_like(rv, dtype=torch.long))))
    rslot = torch.arange(8, device=dev).expand(b, 8)
    out.append(_Section(K["reward"], (rk > 0) & ph(4), rid, cat=rk, owner=rslot + 1,
                        num=[symlog(torch.where(rk == 1, rv, torch.zeros_like(rv))), rfl, rup]))
    rc_idx = (REWARDS_AT + 6 + 21 * torch.arange(8, device=dev).unsqueeze(1)
              + 3 * torch.arange(5, device=dev).unsqueeze(0)).flatten()
    rc = (r.o[:, rc_idx] | (r.o[:, rc_idx + 1] << 8))
    rcu = r.o[:, rc_idx + 2]
    rowner = torch.arange(8, device=dev).repeat_interleave(5).expand(b, 40)
    out.append(_Section(K["reward_card"], (rc > 0) & ph(4), _ids(rc, "cards"),
                        owner=rowner + 1, num=[rcu]))

    # Deck selection.
    sc = r.u16(SELECT_AT + 8, 80, 5)
    su = r.u8(SELECT_AT + 8, 80, 5, 2)
    sfl = r.u8(SELECT_AT + 8, 80, 5, 4)
    sdi = r.u8(SELECT_AT + 8, 80, 5, 3)
    out.append(_Section(K["select"], (sc > 0) & ph(5), _ids(sc, "cards"),
                        num=[su, sfl & 1, (sfl >> 1) & 1, (sdi == 255).float()]))

    # Bundles.
    bc = r.u16(BUNDLES_AT, 16, 3)
    bu = r.u8(BUNDLES_AT, 16, 3, 2)
    bowner = torch.arange(4, device=dev).repeat_interleave(4).expand(b, 16)
    out.append(_Section(K["bundle_card"], (bc > 0) & ph(6), _ids(bc, "cards"),
                        owner=bowner + 1, num=[bu]))

    # Rest site, chest.
    rest_k = r.u8(REST_AT, 8, 2)
    rest_en = r.u8(REST_AT, 8, 2, 1)
    out.append(_Section(K["rest"], (rest_k > 0) & ph(7), cat=rest_k, num=[rest_en]))
    tr = r.u16(TREASURE_AT)
    out.append(_Section(K["treasure"], ph(8).expand(b, 1), _ids(tr, "relics")))

    # Shop.
    shc = r.u16(SHOP_AT, 8, 6)
    shu = r.u8(SHOP_AT, 8, 6, 2)
    shs = r.u8(SHOP_AT, 8, 6, 3)
    shp = r.i16(SHOP_AT, 8, 6, 4)
    gold = r.i16(12)
    out.append(_Section(K["shop_card"], (shc > 0) & ph(9), _ids(shc, "cards"),
                        num=[shu, shs, symlog(shp), (shp <= gold).float()]))
    srel = r.u16(SHOP_AT + 48, 4, 4)
    srp = r.i16(SHOP_AT + 48, 4, 4, 2)
    out.append(_Section(K["shop_relic"], (srel > 0) & ph(9), _ids(srel, "relics"),
                        num=[symlog(srp), (srp <= gold).float()]))
    spot = r.u16(SHOP_AT + 64, 4, 4)
    spp = r.i16(SHOP_AT + 64, 4, 4, 2)
    out.append(_Section(K["shop_potion"], (spot > 0) & ph(9), _ids(spot, "potions"),
                        num=[symlog(spp), (spp <= gold).float()]))
    rem_p = r.i16(SHOP_AT + 80)
    rem_used = r.u8(SHOP_AT + 82)
    out.append(_Section(K["shop_remove"], ph(9).expand(b, 1),
                        num=[symlog(rem_p), rem_used, (rem_p <= gold).float()]))

    if full:
        # The rest of the map, one token per row ahead the window leaves out, outside fights.
        far_ok = (ptype > 0) & (ahead > 0) & ~combat.unsqueeze(1) & ~map_ok
        out.append(_map_rows(far_ok, ptype, ahead, col, child))
        # Pile cards, one token per distinct card with its count in each pile.
        out.append(_pile_section(cards_kind, cards_up, cards_pile, cards_cost, cards_flags,
                                 combat))
    return out, dict(phase=phase, combat=combat, creature_ok=creature_ok, side=side,
                     map_ok=map_ok, col=col, pflags=pflags,
                     cards=(cards_kind, cards_up, cards_pile), deck=(deck, dup))


def _pile_section(kind, up, pile, cost, flags, combat):
    """The draw, discard and exhaust piles' cards as `pile` tokens (module doc): one per distinct
    (card, upgrade, cost, flags), sorted by them, with the copies in each pile counted."""
    b, n = kind.shape
    ok = (kind > 0) & (pile >= 2) & (pile <= 4) & combat.unsqueeze(1)
    key = (((kind.long() * 256 + up.long().clamp(0, 255)) * 256 + (cost.long() + 128)) * 64
           + flags.long())
    key = torch.where(ok, key, torch.full_like(key, 1 << 62))
    skey, order = torch.sort(key, dim=1, stable=True)
    sok = torch.gather(ok, 1, order)
    first = sok & torch.cat([torch.ones_like(sok[:, :1]), skey[:, 1:] != skey[:, :-1]], 1)
    group = (torch.cumsum(first.long(), 1) - 1).clamp(min=0)  # each card's token
    # Per token, its cards' slot (any of them: they are the same bytes) and per-pile counts.
    slot = torch.zeros(b, n + 1, dtype=torch.long, device=kind.device)
    slot.scatter_(1, torch.where(first, group, n), torch.where(first, order, 0))
    slot = slot[:, :n]
    valid = torch.arange(n, device=kind.device) < first.sum(1, keepdim=True)
    which = (torch.gather(pile, 1, order).long() - 2).clamp(0, 2)
    counts = torch.zeros(b, n * 3, device=kind.device)
    counts.scatter_add_(1, group * 3 + which, sok.float())
    counts = counts.view(b, n, 3)
    g = lambda x: torch.where(valid, torch.gather(x, 1, slot), 0)  # noqa: E731
    kind, up, cost, flags = g(kind), g(up), g(cost), g(flags)
    bits = [((flags >> k) & 1) for k in range(6)]
    mask = sum((counts[:, :, k] > 0).long() << k for k in range(3))
    return _Section(K["pile"], valid, _ids(kind, "cards"), cat=mask,
                    num=[up, cost, (cost < 0).float()] + bits
                    + [symlog(counts[:, :, k]) for k in range(3)])


def _map_rows(far, ptype, ahead, col, child):
    """The map points in `far` as `map_row` tokens (module doc): one per row ahead, in order;
    per column its point's type and edges as `hcat` slots."""
    b = far.shape[0]
    dev = far.device
    slot = (ahead.long() - 1).clamp(0, GRID_ROWS - 1)
    c = col.long().clamp(0, MAP_COLS - 1)
    ch = child.long()
    near = sum(((ch >> (c + d).clamp(0, 7)) & 1) * ((c + d >= 0) & (c + d < 8)).long() << k
               for k, d in enumerate((-1, 0, 1)))
    span = sum(1 << k for k in range(8))
    adj = sum(((c + d >= 0) & (c + d < 8)).long() << (c + d).clamp(0, 7) for d in (-1, 0, 1))
    other = ((ch & (span - adj)) != 0).long()
    edges = near | (other << 3)
    cells = torch.zeros(b, GRID_ROWS * MAP_COLS * 2 + 1, dtype=torch.long, device=dev)
    at = slot * MAP_COLS + c
    dump = GRID_ROWS * MAP_COLS * 2
    tcode = MAP_TYPE_AT + c * MAP_TYPES + ptype.long().clamp(0, MAP_TYPES - 1)
    ecode = MAP_EDGE_AT + c * 16 + edges
    cells.scatter_(1, torch.where(far, at * 2, dump), torch.where(far, tcode, 0))
    cells.scatter_(1, torch.where(far, at * 2 + 1, dump), torch.where(far, ecode, 0))
    cells = cells[:, :dump].view(b, GRID_ROWS, MAP_COLS * 2)
    # Slots: column c's type at 2c, its edges at 2c + 1.
    valid = (cells > 0).any(2)
    rows_ahead = torch.arange(1, GRID_ROWS + 1, device=dev).expand(b, GRID_ROWS)
    count = (cells[:, :, 0::2] > 0).sum(2)
    sec = _Section(K["map_row"], valid, num=[rows_ahead * valid, count])
    sec.full = dict(hcat=cells)
    return sec


def _state_num(r, mask_bits):
    """The STATE token's numbers."""
    h = [r.u8(2), r.u8(6), r.u8(7), symlog(r.i16(8)), symlog(r.i16(10)),
         r.i16(8).float() / r.i16(10).clamp(min=1).float(), symlog(r.i16(12)), r.u8(14),
         r.u8(15), symlog(r.u16(16)), r.u8(18), r.u8(19), r.u8(24) * (r.u8(24) != 255),
         r.u8(25) * (r.u8(25) != 255)]
    c = COMBAT_AT
    h += [symlog(r.i16(c)), symlog(r.i16(c + 2)), symlog(r.i16(c + 4)), r.i16(c + 6),
          r.i16(c + 8), r.i16(c + 10), symlog(r.u16(c + 12)), r.u8(c + 16), symlog(r.u8(c + 17)),
          symlog(r.u8(c + 18)), symlog(r.u8(c + 19)), r.u8(c + 20), r.u8(c + 21),
          r.u8(c + 22), r.u8(c + C_CHOICE + 20), r.u8(c + C_CHOICE + 21),
          r.u8(c + C_CHOICE + 22)]
    s = SELECT_AT
    h += [r.u8(s + 1), r.u8(s + 2), r.u8(s + 3), r.u8(s + 4)]
    # A one-hot of the selection purpose (17) and the room type (11) fill the rest.
    purpose = r.u8(s)[:, 0]
    room = r.u8(26)[:, 0]
    cols = [x.float().reshape(x.shape[0], -1)[:, 0] for x in h]
    left = NUM - len(cols)
    oh = torch.zeros(r.o.shape[0], left, device=r.dev)
    np_ = min(17, left - 1)
    oh[:, :np_] = torch.nn.functional.one_hot(purpose.long().clamp(0, np_ - 1), np_).float()
    oh[:, :np_] *= (purpose > 0).float().unsqueeze(1)
    nr = left - np_
    oh[:, np_:] = torch.nn.functional.one_hot(room.long().clamp(0, nr - 1), nr).float()
    return torch.cat([torch.stack(cols, 1), oh], 1)


def mask_bits(mask_words, discard=False):
    """(B, 6) int64 mask words -> (B, 355) bool, or (B, 361) with `discard` (RLN32: the discard-potion
    indices; off, the shapes of before, so the same kernels and bits)."""
    shifts = torch.arange(64, device=mask_words.device, dtype=torch.int64)
    return ((mask_words.unsqueeze(-1) >> shifts) & 1).flatten(1)[:, :(ACTIONS_ALL if discard else ACTIONS)].bool()


def legal_counts(mask_words, discard=False):
    """RLN44: `mask_bits(...).sum(1)` on the host: (B, 6) uint64/int64 numpy mask words -> the
    legal actions per row (int64), so the policy head can size its pair list without a sync."""
    bits = np.unpackbits(np.ascontiguousarray(mask_words).view(np.uint8), axis=1,
                         bitorder="little")
    return bits[:, :(ACTIONS_ALL if discard else ACTIONS)].sum(1, dtype=np.int64)


_TABLES = {}


def _action_tables(device):
    """Per index: (entity kind, slot, sub) from `sts2.action_table()`, on `device`."""
    key = str(device)
    if key not in _TABLES:
        t = torch.as_tensor(_env.action_table().astype("int64"), device=device)
        _TABLES[key] = t
    return _TABLES[key]


_COLS = {}


def _table_cols(device):
    """The action indices of each entity kind, the map columns and the targeted indices, as
    index tensors on `device` (built once on the host, so `encode` never syncs on them)."""
    key = str(device)
    if key not in _COLS:
        t = torch.as_tensor(_env.action_table().astype("int64"))
        ek, sub = t[:, 0], t[:, 2]
        cols = {name: (ek == i).nonzero().squeeze(1) for name, i in _EK.items()}
        cols = {k: v.to(device) for k, v in cols.items() if v.numel()}
        tcols = (((ek == _EK["hand_card"]) | (ek == _EK["combat_potion"])) & (sub >= 1))
        cols["_targeted"] = tcols.nonzero().squeeze(1).to(device)
        _COLS[key] = cols
    return _COLS[key]


# Entity kind (`sts2.env.ENTITY_KINDS`) -> (token section, how the slot indexes it).
_EK = {name: i for i, name in enumerate(_env.ENTITY_KINDS)}


def encode(obs, mask_words, v2=False, full=False, events=False, discard=False):
    """A batch of run observations (uint8 B x 2,996, or wider; `v2` reads the v2 section, B x
    `OBS_BYTES_V2`, and `full` with it the full view, B x `OBS_BYTES_FULL`) and mask words (int64
    B x 6) -> a dict of token tensors (see the module doc), all on `obs`'s device. The v1 tokens
    do not depend on `v2`, and nothing depends on `full` or `events` when it is off; `events`
    reads the event section (B x `OBS_BYTES_EVENTS`). `legal` is 355 wide, or 361 with `discard`
    (the net's `n_actions`)."""
    if full and not v2:
        raise ValueError("the full view needs v2")
    if events and not v2:
        raise ValueError("the event section needs v2")
    b = obs.shape[0]
    dev = obs.device
    tm = t_max(full)
    r = _Bytes(obs[:, :obs_width(v2, full, events)])
    legal = mask_bits(mask_words, discard)
    secs, aux = _sections(r, legal, full)
    extra = _v2_extras(r, legal, secs) if v2 else None
    if full:
        _full_extras(r, secs)
    if events:
        ev = _event_extras(r, secs)

    # Candidate tensors in section order.
    valid = torch.cat([s.valid for s in secs], 1)
    kind = torch.cat([torch.full((b, s.n), s.kind, dtype=torch.long, device=dev) for s in secs],
                     1)
    ident = torch.cat([s.ident for s in secs], 1)
    cat = torch.cat([s.cat for s in secs], 1)
    owner = torch.cat([s.owner for s in secs], 1)
    num = torch.cat([_numcat(s.num, b, s.n, dev) for s in secs], 1)
    starts, at = {}, 0
    for s in secs:
        starts.setdefault(s.kind, at)
        at += s.n
    c = valid.shape[1]

    # Compaction: valid candidates keep their order, from position FIXED.
    rank = torch.cumsum(valid.long(), 1) - 1
    count = valid.sum(1)
    keep = valid & (rank < tm - FIXED)
    pos_of = torch.where(keep, rank + FIXED, torch.zeros_like(rank))  # dropped -> STATE
    order = torch.argsort((~keep).to(torch.int8), dim=1, stable=True)[:, : tm - FIXED]
    g = lambda t: torch.gather(t, 1, order)  # noqa: E731
    tok_valid = g(keep)
    tok_kind = torch.where(tok_valid, g(kind), torch.zeros_like(g(kind)))
    tok_ident = torch.where(tok_valid, g(ident), torch.zeros_like(g(ident)))
    tok_cat = torch.where(tok_valid, g(cat), torch.zeros_like(g(cat)))
    tok_owner = torch.where(tok_valid, g(owner), torch.zeros_like(g(owner)))
    tok_num = torch.gather(num, 1, order.unsqueeze(-1).expand(-1, -1, NUM))
    tok_num = tok_num * tok_valid.unsqueeze(-1)

    # Fixed positions: STATE and the four summaries.
    phase = aux["phase"]
    combat = aux["combat"]
    ck, cu, cp = aux["cards"]
    dk, du = aux["deck"]
    bag_id = torch.where(combat.unsqueeze(1), torch.cat([ck, torch.zeros_like(ck[:, :16])], 1),
                         dk)
    bag_up = torch.where(combat.unsqueeze(1), torch.cat([cu, torch.zeros_like(cu[:, :16])], 1),
                         du)
    pile_group = torch.where((cp >= 2) & (cp <= 4), cp - 1, torch.zeros_like(cp))
    bag_group = torch.where(combat.unsqueeze(1),
                            torch.cat([pile_group, torch.zeros_like(cp[:, :16])], 1),
                            (dk > 0).to(torch.int32) * 4)
    bag_group = torch.where(bag_id > 0, bag_group, torch.zeros_like(bag_group))
    counts = torch.stack([(bag_group == k).sum(1) for k in range(1, 5)], 1)
    fixed_valid = torch.cat([torch.ones(b, 1, dtype=torch.bool, device=dev), counts > 0], 1)
    fixed_kind = torch.arange(FIXED, device=dev).expand(b, FIXED)
    fixed_cat = torch.cat([phase.long().unsqueeze(1), torch.zeros(b, 4, dtype=torch.long,
                                                                  device=dev)], 1)
    fixed_num = torch.zeros(b, FIXED, NUM, device=dev)
    fixed_num[:, 0] = _state_num(r, legal)
    fixed_num[:, 1:, 0] = symlog(counts)
    fixed_num[:, 1:, 1] = (counts > 0).float()
    zeros = torch.zeros(b, FIXED, dtype=torch.long, device=dev)

    out = dict(
        valid=torch.cat([fixed_valid, tok_valid], 1),
        kind=torch.cat([fixed_kind, tok_kind], 1),
        ident=torch.cat([zeros, tok_ident], 1),
        cat=torch.cat([fixed_cat, tok_cat], 1),
        owner=torch.cat([zeros, tok_owner], 1),
        num=torch.cat([fixed_num, tok_num], 1),
        bag_id=_ids(bag_id, "cards"),
        bag_up=bag_up.float(),
        bag_group=bag_group.long(),
        phase=phase.long(),
        combat=combat,
        overflow=(count > tm - FIXED),
        legal=legal,
    )
    out["count"] = out["valid"].sum(1)
    out["width"] = FIXED + keep.sum(1)
    if extra is not None:
        xnum = torch.cat([_numcat(s.xnum, b, s.n, dev)[:, :, :XNUM] for s in secs], 1)
        xcat = torch.cat([torch.zeros(b, s.n, 3, dtype=torch.long, device=dev)
                          if s.xcat is None else s.xcat.long() for s in secs], 1)
        link = torch.cat([torch.zeros(b, s.n, dtype=torch.long, device=dev)
                          if s.link is None else s.link.long() for s in secs], 1)
        tv = tok_valid.unsqueeze(-1)
        fixed_x = torch.zeros(b, FIXED, XNUM, device=dev)
        fixed_x[:, 0] = extra["state"]
        out["xnum"] = torch.cat(
            [fixed_x, torch.gather(xnum, 1, order.unsqueeze(-1).expand(-1, -1, XNUM)) * tv], 1)
        out["xcat"] = torch.cat(
            [torch.zeros(b, FIXED, 3, dtype=torch.long, device=dev),
             torch.where(tv, torch.gather(xcat, 1, order.unsqueeze(-1).expand(-1, -1, 3)), 0)],
            1)
        out["link"] = torch.cat([zeros, torch.where(tok_valid, g(link), 0)], 1)
        out["y_valid"], out["y_cat"], out["y_num"] = extra["y"]
        # Only the creature and power sections have side columns: fill their slices.
        znum = torch.zeros(b, sum(s.n for s in secs), ZNUM, device=dev)
        z_at = 0
        for sec in secs:
            for z_col, z in enumerate(sec.znum):
                znum[:, z_at:z_at + sec.n, z_col] = z.float()
            z_at += sec.n
        out["znum"] = torch.cat(
            [torch.zeros(b, FIXED, ZNUM, device=dev),
             torch.gather(znum, 1, order.unsqueeze(-1).expand(-1, -1, ZNUM)) * tv], 1)
        out["anum"] = plan_columns(r, legal.shape[1])
    if full:
        tv = tok_valid.unsqueeze(-1)
        for name, width, dtype in (("hcat", HCAT, torch.long), ("hmove", HIST, torch.long),
                                   ("hnum", HNUM, torch.float32)):
            cand = torch.zeros(b, c, width, dtype=dtype, device=dev)
            at = 0
            for sec in secs:
                if sec.full is not None and name in sec.full:
                    x = sec.full[name]
                    cand[:, at:at + sec.n, :x.shape[-1]] = x.to(dtype)
                at += sec.n
            got = torch.gather(cand, 1, order.unsqueeze(-1).expand(-1, -1, width))
            out[name] = torch.cat([torch.zeros(b, FIXED, width, dtype=dtype, device=dev),
                                   torch.where(tv, got, torch.zeros_like(got))], 1)
    if events:
        slot = torch.cat([torch.zeros(b, s.n, dtype=torch.long, device=dev) if s.ev is None
                          else (torch.arange(1, s.n + 1, device=dev)
                                * s.ev.to(torch.long)).expand(b, s.n) for s in secs], 1)
        out["ev_slot"] = torch.cat([zeros, torch.where(tok_valid, g(slot), 0)], 1)
        out |= ev

    # Action entities.
    table = _action_tables(dev)
    ek, slot, sub = table[:, 0], table[:, 1], table[:, 2]

    def at_cand(section, idx):
        """Positions of candidates `starts[section] + idx` ((B, A) or (A,) idx)."""
        idx = idx if idx.dim() == 2 else idx.expand(b, -1)
        return torch.gather(pos_of, 1, (starts[section] + idx).clamp(0, c - 1))

    ent = torch.zeros(b, ACTIONS_ALL, dtype=torch.long, device=dev)

    tcols_of = _table_cols(dev)

    def put(entity_kind, section, idx):
        cols = tcols_of.get(entity_kind)
        if cols is None:
            return
        ent[:, cols] = at_cand(section, idx[cols].unsqueeze(0).expand(b, -1))

    put("hand_card", K["hand"], slot)
    put("combat_potion", K["belt"], slot)
    put("belt_potion", K["belt"], slot)
    put("discard_potion", K["belt"], slot)
    put("choice_option", K["choice"], slot)
    put("ancient_option", K["ancient_option"], slot)
    put("event_option", K["event_option"], slot)
    put("reward", K["reward"], slot)
    put("reward_alternative", K["reward"], slot)
    put("reward_skip", K["reward"], slot)
    put("reward_card", K["reward_card"], slot * 5 + sub)
    put("rest_option", K["rest"], slot)
    put("select_option", K["select"], slot)
    put("treasure_relic", K["treasure"], torch.zeros_like(slot))
    put("shop_card", K["shop_card"], slot)
    put("shop_relic", K["shop_relic"], slot)
    put("shop_potion", K["shop_potion"], slot)
    put("shop_remove", K["shop_remove"], torch.zeros_like(slot))
    put("bundle", K["bundle_card"], slot * 4)
    # Map column c -> the legal next point in column c.
    nxt = aux["map_ok"] & (((aux["pflags"] >> 2) & 1) == 1)
    cols8 = torch.arange(8, device=dev)
    match = nxt.unsqueeze(1) & (aux["col"].unsqueeze(1) == cols8.view(1, 8, 1))  # B,8,96
    has = match.any(2)
    point = match.to(torch.int8).argmax(2)
    map_pos = torch.gather(pos_of, 1, starts[K["map"]] + point)
    map_pos = torch.where(has, map_pos, torch.zeros_like(map_pos))
    mcols = tcols_of["map_column"]
    ent[:, mcols] = torch.gather(map_pos, 1, slot[mcols].unsqueeze(0).expand(b, -1).clamp(0, 7))

    # Targets: sub k >= 1 is the k-th enemy (side 1) in creature order.
    enemy = aux["creature_ok"] & (aux["side"] == 1)
    erank = torch.cumsum(enemy.long(), 1) - 1
    ematch = enemy.unsqueeze(1) & (erank.unsqueeze(1) == torch.arange(7, device=dev).view(1, 7, 1))
    ehas = ematch.any(2)
    eslot = ematch.to(torch.int8).argmax(2)
    epos = torch.gather(pos_of, 1, starts[K["creature"]] + eslot)
    epos = torch.where(ehas, epos, torch.full_like(epos, -1))
    tgt = torch.full((b, ACTIONS_ALL), -1, dtype=torch.long, device=dev)
    tcols = tcols_of["_targeted"]
    tgt[:, tcols] = torch.gather(epos, 1, (sub[tcols] - 1).clamp(0, 6).unsqueeze(0)
                                 .expand(b, -1))
    out["ent"] = ent
    out["tgt"] = tgt
    out["akind"] = ek
    out["asub"] = sub
    return out



# ---- v2 (`obs_v2.rs`, RLN16) ----

XNUM = 16
"""v2 numeric columns per token."""
FC_KINDS = 32
"""Forecast kind rows per step in the net's `x_fc` table (kind + 1 < 32)."""
MAPNODES = 40
"""Reachable map points shown as v2 tokens."""
GRID_ROWS = 20
"""Map rows the route maths covers (an act has about 17)."""
STEPS, STEP_BYTES, PREVIEW_BYTES = 3, 7, 6
V_LINK, V_FORECAST, V_PREVIEW, V_AIMED = 0, 8, 176, 236
V_SIDE, V_DISPLAY = 396, 412
"""RLN21: per creature (facing, arm), per creature and power (shown, number `i16`)."""
SIDE_BYTES, DISPLAY_BYTES = 2, 3
ZNUM = 4
"""RLN21 numeric columns per token (`znum`)."""
V_PLAN, PLAN_BYTES, COMBAT_ACTIONS = 700, 3, 140
"""RLN22: per combat action index (`action.rs`, the first 140 of the space), the plan column's
score (`i16`) and flags (bit 0 scored, 1 the planner's choice, 2 a winning play)."""
ANUM = 5
"""RLN22 numeric columns per action index (`anum`)."""


def plan_columns(r, actions=ACTIONS):
    """The plan column of `r`'s rows as `anum` (B x `actions` x `ANUM`, module doc); zero for
    the run indices and the unscored ones."""
    at = V2_AT + V_PLAN
    flags = r.u8(at, COMBAT_ACTIONS, PLAN_BYTES, 2)
    score = r.i16(at, COMBAT_ACTIONS, PLAN_BYTES, 0)
    scored = (flags & 1) == 1
    best = torch.where(scored, score, torch.full_like(score, -(1 << 20))).amax(1, keepdim=True)
    s = scored.float()
    cols = torch.stack([s, ((flags >> 1) & 1).float() * s, ((flags >> 2) & 1).float() * s,
                        symlog(score / 10) * s, symlog((score - best) / 10) * s], -1)
    b = cols.shape[0]
    return torch.cat([cols, torch.zeros(b, actions - COMBAT_ACTIONS, ANUM, device=cols.device)],
                     1)
ENERGY_CAP = 15
"""The knapsack's largest energy (more is clamped)."""
# `MapPointType` + 1 as the obs stores it.
PT_UNKNOWN, PT_SHOP, PT_TREASURE, PT_REST, PT_MONSTER, PT_ELITE, PT_BOSS = 2, 3, 4, 5, 6, 7, 8

_HAND_COLS = {}


def _hand_play(device):
    """(hand_card action indices, (n, 10) one-hot of their hand slot), on `device`."""
    key = str(device)
    if key not in _HAND_COLS:
        t = torch.as_tensor(_env.action_table().astype("int64"))
        cols = (t[:, 0] == _EK["hand_card"]).nonzero().squeeze(1)
        hot = torch.nn.functional.one_hot(t[cols, 1].clamp(0, 9), 10).float()
        _HAND_COLS[key] = (cols.to(device), hot.to(device))
    return _HAND_COLS[key]


def _knapsack(value, cost, energy):
    """Per row and target, the most of `value` (B x 10 x 8 int) the cards with `cost` (B x 10
    int: 0 free, -1 never, else energy) fit in `energy` (B,)."""
    b = value.shape[0]
    caps = torch.arange(ENERGY_CAP + 1, device=value.device)
    dp = torch.zeros(b, 8, ENERGY_CAP + 1, dtype=torch.int32, device=value.device)
    free = torch.zeros(b, 8, dtype=torch.int32, device=value.device)
    for i in range(10):
        v, k = value[:, i], cost[:, i]  # (B, 8), (B,)
        free = free + torch.where((k == 0).unsqueeze(1), v, 0)
        kk = k.clamp(1, ENERGY_CAP + 1).view(b, 1, 1)
        src = (caps.view(1, 1, -1) - kk).clamp(min=0).expand(b, 8, -1)
        take = torch.gather(dp, 2, src) + v.unsqueeze(-1)
        fits = (caps.view(1, 1, -1) >= kk) & (k > 0).view(b, 1, 1)
        dp = torch.where(fits, torch.maximum(dp, take), dp)
    at = energy.clamp(0, ENERGY_CAP).long().view(b, 1, 1).expand(b, 8, 1)
    return torch.gather(dp, 2, at).squeeze(-1) + free


def _route(ptype, row, col, child, nxt, b, dev):
    """spire-mentor's `_route` and `_paths_through` over the act map on a GRID_ROWS x 8 grid:
    per cell (B x GRID_ROWS*8), the stats over the paths from it to the boss (elites min/max,
    rests min/max, most shops, most unknowns, fewest fights before a rest; the cell included),
    the rows to the boss row, and the paths from the legal next points through it."""
    n = GRID_ROWS * 8
    node = (ptype >= PT_UNKNOWN) & (ptype <= PT_ELITE) & (row < GRID_ROWS)
    cell = torch.where(node, row.long() * 8 + col.long().clamp(0, 7), n)

    def grid(x):
        g = torch.zeros(b, n + 1, dtype=x.dtype, device=dev)
        return g.scatter(1, cell, x)[:, :n].view(b, GRID_ROWS, 8)

    is_node, typ, kids = grid(node), grid(ptype), grid(child)
    start = grid(nxt & node)
    boss = ptype == PT_BOSS
    boss_row = torch.where(boss, row, 0).amax(1)
    bits = 1 << torch.arange(8, device=dev)
    km = ((kids.unsqueeze(-1) & bits) != 0)  # B, R, 8 cols, 8 kid cols
    km[:, :-1] &= is_node[:, 1:].unsqueeze(2)
    km[:, -1] = False
    e = (typ == PT_ELITE).float()
    rest = (typ == PT_REST).float()
    shop = (typ == PT_SHOP).float()
    unk = (typ == PT_UNKNOWN).float()
    fight = ((typ == PT_MONSTER) | (typ == PT_ELITE)).float()
    own = torch.stack([e, e, rest, rest, shop, unk], -1)  # B, R, 8, 6
    is_min = torch.tensor([True, False, True, False, False, False], device=dev)
    big = 1e9
    stats = [None] * GRID_ROWS
    f2r = [None] * GRID_ROWS
    for r_ in range(GRID_ROWS - 1, -1, -1):
        m = km[:, r_].unsqueeze(-1)  # B, 8, 8, 1
        has = km[:, r_].any(-1).unsqueeze(-1)  # B, 8, 1
        if r_ + 1 < GRID_ROWS:
            k = stats[r_ + 1].unsqueeze(1)  # B, 1, 8, 6
            kmin = torch.where(m, k, big).amin(2)
            kmax = torch.where(m, k, -big).amax(2)
            kid = torch.where(is_min, kmin, kmax)
            kf = torch.where(m.squeeze(-1), f2r[r_ + 1].unsqueeze(1), big).amin(2)
        else:
            kid = torch.zeros(b, 8, 6, device=dev)
            kf = torch.zeros(b, 8, device=dev)
        st = own[:, r_] + torch.where(has, kid, 0.0)
        fr = torch.where(rest[:, r_] > 0, 0.0,
                         fight[:, r_] + torch.where(has.squeeze(-1), kf, 0.0))
        live = is_node[:, r_]
        stats[r_] = torch.where(live.unsqueeze(-1), st, 0.0)
        f2r[r_] = torch.where(live, fr, 0.0)
    rows = torch.arange(GRID_ROWS, device=dev).view(1, GRID_ROWS, 1)
    left = (boss_row.view(b, 1, 1) - rows).float().expand(b, GRID_ROWS, 8)
    route = torch.cat([torch.stack(stats, 1), torch.stack(f2r, 1).unsqueeze(-1),
                       left.unsqueeze(-1)], -1) * is_node.unsqueeze(-1)  # B, R, 8, 8
    cnt = [start[:, 0].float()]
    for r_ in range(GRID_ROWS - 1):
        into = (cnt[-1].unsqueeze(-1) * km[:, r_].float()).sum(1)
        cnt.append(start[:, r_ + 1].float() + into)
    paths = torch.stack(cnt, 1) * is_node
    return route.view(b, n, 8), paths.view(b, n), typ.view(b, n), is_node.view(b, n), cell


@torch.compiler.disable
def _ratio(most, need):
    """`most / need` capped at 2, eager: inductor's division may round otherwise."""
    return (most.float() / need.clamp(min=1).float()).clamp(max=2)


_V2_COMPILED = None
"""`_v2` compiled by `enable_compile` (the same values, fewer kernel launches), else None."""


def enable_compile():
    """Compiles the v2 extras' maths (`_v2`, inductor, dynamic batch) for `encode`: about 600
    small kernels become a few dozen, the same values bit for bit. Idempotent."""
    global _V2_COMPILED
    if _V2_COMPILED is None:
        _V2_COMPILED = torch.compile(_v2, dynamic=True)


def disable_compile():
    global _V2_COMPILED
    _V2_COMPILED = None


def _v2_extras(r, legal, secs):
    """`_v2`, compiled when `enable_compile` was called; a compiled build that fails turns
    compilation off and runs eager."""
    if _V2_COMPILED is not None:
        try:
            return _V2_COMPILED(r, legal, secs)
        except Exception as e:  # noqa: BLE001 - any failure of the compiled build means eager
            print(f"encode: torch.compile of the v2 extras failed, running eager: "
                  f"{type(e).__name__}: {str(e)[:300]}", flush=True)
            disable_compile()
    return _v2(r, legal, secs)


POWER_DOOM, POWER_DIE_FOR_YOU = 59, 57
"""`crates/sts2-rl/vocab/powers.txt` ids (line - 1) of `DOOM_POWER` and `DIE_FOR_YOU_POWER`."""


def _osty_absorb(pkind, alive, side, hp, block):
    """RLN46: the damage aimed at the player that a living ally with `DieForYou` (Osty) takes
    first (`DieForYouPower.cs`: unblocked powered damage goes to Osty, the overkill comes
    back), i.e. that ally's HP plus block; B."""
    slot = torch.arange(hp.shape[1], device=hp.device)
    pet = alive & (side == 0) & (slot > 0) & (pkind == POWER_DIE_FOR_YOU).any(-1)
    return ((hp + block).clamp(min=0) * pet).sum(1)


def _doomed(pkind, pamt, alive, side, hp, block, most):
    """RLN46: per creature, an enemy whose Doom kills it at the end of its turn if this hand's
    best damage on it lands first (`DoomPower.IsOwnerDoomed`: HP <= Doom); B x 8."""
    doom = (pamt * (pkind == POWER_DOOM)).sum(-1)
    after = hp - (most - block).clamp(min=0)
    return alive & (side == 1) & (doom > 0) & (after <= doom)


def _v2(r, legal, secs):
    """Attaches the v2 extras to `secs` (module doc); returns the STATE token's columns and the
    MAPNODE tokens."""
    b = r.o.shape[0]
    dev = r.dev
    by = {}
    for s in secs:
        by.setdefault(s.kind, s)  # the full view's far map section comes after the window's
    v = V2_AT
    combat = (r.u8(3)[:, 0] == PHASE_COMBAT).unsqueeze(1)
    # T1: the intents' enemies and the creatures' forecasts.
    by[K["intent"]].link = r.u8(v + V_LINK, 8) * combat
    fk = r.u8(v + V_FORECAST, 24, STEP_BYTES, 0).view(b, 8, STEPS)
    fc = r.u8(v + V_FORECAST, 24, STEP_BYTES, 1).view(b, 8, STEPS).float() / 255
    fd = r.i16(v + V_FORECAST, 24, STEP_BYTES, 2).view(b, 8, STEPS)
    fh = r.u8(v + V_FORECAST, 24, STEP_BYTES, 4).view(b, 8, STEPS)
    ft = r.i16(v + V_FORECAST, 24, STEP_BYTES, 5).view(b, 8, STEPS)
    steps = torch.arange(STEPS, device=dev) * FC_KINDS
    by[K["creature"]].xcat = (fk.clamp(0, FC_KINDS - 1) + steps) * combat.unsqueeze(-1)
    # T2: previews, the knapsack, the HP lost now.
    pd = r.i16(v + V_PREVIEW, 10, PREVIEW_BYTES, 0)
    pb = r.i16(v + V_PREVIEW, 10, PREVIEW_BYTES, 2)
    ph = r.u8(v + V_PREVIEW, 10, PREVIEW_BYTES, 4)
    pf = r.u8(v + V_PREVIEW, 10, PREVIEW_BYTES, 5)
    aimed = r.i16(v + V_AIMED, 80, 2).view(b, 10, 8)
    cols, hot = _hand_play(dev)
    playable = (legal[:, cols].float() @ hot) > 0  # B, 10
    hand_ids = r.u16(COMBAT_AT + C_HAND, 10).long().clamp(0, 63)
    cost = torch.gather(r.i8(COMBAT_AT + C_CARDS, 64, 6, 4), 1, hand_ids)
    energy = r.i16(COMBAT_AT + 6)[:, 0]
    # An X cost spends all the energy; an unplayable card never fits.
    kcost = torch.where(cost < 0, energy.unsqueeze(1), cost)
    kcost = torch.where(playable, kcost, torch.full_like(kcost, ENERGY_CAP + 1))
    value = aimed * ph.unsqueeze(-1) * playable.unsqueeze(-1)
    most = _knapsack(value, kcost, energy)  # B, 8
    by[K["hand"]].xnum = [symlog(pd), symlog(pb), ph, pf & 1, (pf >> 1) & 1, symlog(pd * ph),
                          playable]
    cb = COMBAT_AT + C_CREATURES
    alive = r.u8(cb, 8, CREATURE_BYTES, 0) > 0
    side = r.u8(cb, 8, CREATURE_BYTES, 1)
    hp = r.i16(cb, 8, CREATURE_BYTES, 6)
    block = r.i16(cb, 8, CREATURE_BYTES, 10)
    need = hp + block
    killable = alive & (side == 1) & (need > 0) & (most >= need) & combat
    pkind = torch.stack([r.u16(cb, 8, CREATURE_BYTES, 12 + 4 * j) for j in range(12)], -1)
    pamt = torch.stack([r.i16(cb, 8, CREATURE_BYTES, 14 + 4 * j) for j in range(12)], -1)
    doomed = _doomed(pkind, pamt, alive, side, hp, block, most) & combat
    by[K["creature"]].xnum = (
        [fc[:, :, k] for k in range(STEPS)] + [symlog(fd[:, :, k]) for k in range(STEPS)]
        + [fh[:, :, k] for k in range(STEPS)] + [symlog(ft[:, :, k]) for k in range(STEPS)]
        + [symlog(most), killable, _ratio(most, need), doomed])
    idmg = r.i16(COMBAT_AT + C_INTENTS, 8, 5, 1)
    ihits = r.u8(COMBAT_AT + C_INTENTS, 8, 5, 3)
    incoming = (idmg * ihits).sum(1)
    absorb = _osty_absorb(pkind, alive, side, hp, block)
    lost = (incoming - r.i16(COMBAT_AT + 4)[:, 0] - absorb).clamp(min=0) * combat[:, 0]
    state = torch.zeros(b, XNUM, device=dev)
    state[:, 0] = symlog(lost)
    state[:, 1] = ((lost >= r.i16(COMBAT_AT)[:, 0]) & (lost > 0)).float()
    state[:, 2] = killable.any(1).float()
    state[:, 3] = symlog(incoming * combat[:, 0])
    state[:, 4] = doomed.any(1).float()  # RLN46
    state[:, 5] = symlog(absorb * combat[:, 0])
    # RLN21: the facing and the arms behind (creatures), the shown numbers (powers).
    facing = r.u8(v + V_SIDE, 8, SIDE_BYTES, 0) * combat
    arm = r.u8(v + V_SIDE, 8, SIDE_BYTES, 1) * combat
    by[K["creature"]].znum = [facing == 1, facing == 2, arm == 1, arm == 2]
    shown = r.u8(v + V_DISPLAY, 96, DISPLAY_BYTES, 0) * combat
    number = r.i16(v + V_DISPLAY, 96, DISPLAY_BYTES, 1) * shown
    by[K["power"]].znum = [shown, symlog(number)]
    # T3: route summaries on the map window, and the cone as MAPNODE tokens.
    col = r.u8(MAP_AT, 96, 5, 0)
    row = r.u8(MAP_AT, 96, 5, 1)
    ptype = r.u8(MAP_AT, 96, 5, 2)
    pflags = r.u8(MAP_AT, 96, 5, 3)
    child = r.u8(MAP_AT, 96, 5, 4)
    nxt = ((pflags >> 2) & 1) == 1
    route, paths, typ, is_node, cell = _route(ptype, row, col, child, nxt, b, dev)
    n = GRID_ROWS * 8
    at = cell.clamp(max=n - 1)
    on = (cell < n).unsqueeze(-1)
    win = torch.gather(route, 1, at.unsqueeze(-1).expand(-1, -1, 8)) * on
    wpaths = torch.gather(paths, 1, at) * on.squeeze(-1)
    by[K["map"]].xnum = [win[:, :, k] for k in range(8)] + [torch.log1p(wpaths)]
    for s in secs:  # the full view's far map points: the same per-point columns
        if s.kind == K["map"] and s is not by[K["map"]]:
            s.xnum = by[K["map"]].xnum
    reach = paths > 0
    rank = torch.cumsum(reach.long(), 1) - 1
    keep = reach & (rank < MAPNODES)
    order = torch.argsort((~keep).to(torch.int8), dim=1, stable=True)[:, :MAPNODES]
    y_valid = torch.gather(keep, 1, order)
    cells = order
    cur_row = r.u8(25)
    cur = torch.where(cur_row == 255, torch.full_like(cur_row, -1), cur_row).float()
    y_row = (cells // 8).float()
    y_col = (cells % 8).float()
    nxt_grid = torch.zeros(b, n + 1, dtype=torch.bool, device=dev).scatter(1, cell, nxt)[:, :n]
    yr = torch.gather(route, 1, cells.unsqueeze(-1).expand(-1, -1, 8))
    y_num = torch.zeros(b, MAPNODES, XNUM, device=dev)
    y_num[:, :, 0] = y_row - cur
    y_num[:, :, 1] = y_col
    y_num[:, :, 2] = torch.gather(nxt_grid, 1, cells).float()
    y_num[:, :, 3] = torch.log1p(torch.gather(paths, 1, cells))
    y_num[:, :, 4:12] = yr
    y_num = y_num * y_valid.unsqueeze(-1)
    y_cat = torch.where(y_valid, torch.gather(typ, 1, cells).long(), 0)
    return dict(state=state, y=(y_valid, y_cat, y_num))


# ---- Full view (`obs_full.rs`, RLN30) ----

HIST = 3
"""Past moves per creature (`obs_full::HISTORY`)."""
HIST_SLOT, HIST_MOVE = 28, 9
"""Bytes per creature slot and per past move."""
HNUM = 5 * HIST + 1
"""Numeric columns of a creature's past moves per token."""
MAP_COLS, MAP_TYPES = 7, 16
"""Map columns, and room-type rows per column (`ptype` < `MAP_TYPES`), of a `map_row` token."""
MAP_TYPE_AT = HIST * FC_KINDS
"""`hcat` rows: the past moves' first, then per column the point's type, then its edges."""
MAP_EDGE_AT = MAP_TYPE_AT + MAP_COLS * MAP_TYPES
HCAT_ROWS = MAP_EDGE_AT + MAP_COLS * 16
HCAT = 2 * MAP_COLS
"""`hcat` slots per token (a creature uses the first `HIST`, a `map_row` all)."""


def _full_extras(r, secs):
    """Attaches the past moves (module doc) to the creature section of `secs`."""
    b = r.o.shape[0]
    dev = r.dev
    base = FULL_AT + HIST_SLOT * torch.arange(8, device=dev)
    count = r.o[:, base]  # B, 8
    steps = torch.arange(HIST, device=dev)
    at = (base.unsqueeze(1) + 1 + HIST_MOVE * steps.unsqueeze(0)).flatten()  # 8*HIST
    o = r.o

    def u16(i):
        return o[:, i] | (o[:, i + 1] << 8)

    def i16(i):
        v = u16(i)
        return v - ((v >= 32768).to(torch.int32) << 16)

    key = u16(at).view(b, 8, HIST)
    seen = o[:, at + 2].view(b, 8, HIST)
    kind = o[:, at + 3].view(b, 8, HIST)
    dmg = i16(at + 4).view(b, 8, HIST)
    hits = o[:, at + 6].view(b, 8, HIST)
    total = i16(at + 7).view(b, 8, HIST)
    present = key > 0
    hcat = torch.where(present, steps * FC_KINDS + (kind + 1).clamp(max=FC_KINDS - 1), 0)
    hmove = torch.where(present, 1 + steps * MOVE_BUCKETS + key.long() % MOVE_BUCKETS, 0)
    cols = []
    for k in range(HIST):
        cols += [present[:, :, k].float(), seen[:, :, k].float(), symlog(dmg[:, :, k]), hits[:, :, k].float(),
                 symlog(total[:, :, k])]
    cols.append(symlog(count))
    by = next(s for s in secs if s.kind == K["creature"])
    by.full = dict(hcat=hcat, hmove=hmove, hnum=torch.stack(cols, -1))

# ---- Event options (`obs_events.rs`, RLN33) ----

_EV_OFFS = {}


def _ev_offsets(device):
    key = str(device)
    if key not in _EV_OFFS:
        _EV_OFFS[key] = torch.tensor([_OFF[t] for t in EV_ID_TABLES], device=device)
    return _EV_OFFS[key]


def _event_extras(r, secs):
    """The page's option vectors (module doc), and marks the ancient and event option sections
    of `secs` as reading them (a section's token k is option k of the pending page)."""
    b = r.o.shape[0]
    dev = r.dev
    base = EVENTS_AT + EV_OPT * torch.arange(8, device=dev)  # 8
    o = r.o
    cols = base.unsqueeze(1) + 2 * torch.arange(EV_COLS, device=dev)  # 8, C
    v = (o[:, cols] | (o[:, cols + 1] << 8))
    v = v - ((v >= 32768).to(torch.int32) << 16)
    ids_at = base.unsqueeze(1) + 2 * EV_COLS + 2 * torch.arange(EV_IDS, device=dev)
    raw = o[:, ids_at] | (o[:, ids_at + 1] << 8)  # B, 8, I
    eids = torch.where(raw > 0, raw.long() + _ev_offsets(dev), torch.zeros_like(raw.long()))
    ench = o[:, base + 2 * EV_COLS + 2 * EV_IDS].clamp(max=63)
    for s in secs:
        if s.kind in (K["ancient_option"], K["event_option"]):
            s.ev = torch.ones(s.n, dtype=torch.bool, device=dev)
    return dict(evec=symlog(v), eids=eids, ench=ench.long())


# The privileged buffer (`priv_obs.rs`).
PRIV_BYTES = 690
P_DRAW, P_MOVES, P_POINTS, P_NORMALS, P_ELITES, P_EVENTS, P_NEXT = 8, 38, 70, 166, 182, 190, 206
PRIV_KINDS = ("priv_state", "draw", "move", "rooms", "normal", "elite", "event", "next_boss",
              "next_map")
PK = {name: i for i, name in enumerate(PRIV_KINDS)}
PRIV_T = 1 + 10 + 8 + 1 + 8 + 4 + 8 + 2 + 1
MOVE_BUCKETS = 1024
ROOM_TYPES = 16


def encode_priv(priv):
    """Privileged bytes (uint8 B x 690) -> a dict of value-module tokens: `kind`, `ident` (a
    row of the shared ID table, or for moves a hash bucket of the value's own table), `num`
    (B x PRIV_T x NUM), `valid`, and the two room-type grids `rooms` and `next_map`
    (B x 96 int64); `move_next` is each move token's following move (B x PRIV_T)."""
    b = priv.shape[0]
    dev = priv.device
    r = _Bytes(priv)
    in_fight, draw_n, next_act = r.u8(0), r.u8(1), r.u8(2)
    kinds, idents, valids, nums = [], [], [], []

    def add(kind, valid, ident=None, num=()):
        n = valid.shape[1]
        kinds.append(torch.full((b, n), PK[kind], dtype=torch.long, device=dev))
        idents.append(torch.zeros(b, n, dtype=torch.long, device=dev) if ident is None
                      else ident.long())
        valids.append(valid)
        nums.append(_numcat(list(num), b, n, dev))

    add("priv_state", torch.ones(b, 1, dtype=torch.bool, device=dev),
        num=[in_fight, symlog(draw_n), next_act])
    dc = r.u16(P_DRAW, 10, 3)
    du = r.u8(P_DRAW, 10, 3, 2)
    add("draw", dc > 0, _ids(dc, "cards"), num=[du, torch.arange(10, device=dev).expand(b, 10)])
    m_now = r.u16(P_MOVES, 8, 4)
    m_next = r.u16(P_MOVES, 8, 4, 2)
    add("move", m_now > 0, m_now.long() % MOVE_BUCKETS,
        num=[torch.arange(8, device=dev).expand(b, 8)])
    add("rooms", torch.ones(b, 1, dtype=torch.bool, device=dev))
    nm = r.u16(P_NORMALS, 8)
    add("normal", nm > 0, _ids(nm, "encounters"), num=[torch.arange(8, device=dev).expand(b, 8)])
    el = r.u16(P_ELITES, 4)
    add("elite", el > 0, _ids(el, "encounters"), num=[torch.arange(4, device=dev).expand(b, 4)])
    ev = r.u16(P_EVENTS, 8)
    add("event", ev > 0, _ids(ev, "events"), num=[torch.arange(8, device=dev).expand(b, 8)])
    nb = r.u16(P_NEXT, 2)
    add("next_boss", nb > 0, _ids(nb, "encounters"))
    add("next_map", (next_act > 0))
    return dict(
        kind=torch.cat(kinds, 1), ident=torch.cat(idents, 1), valid=torch.cat(valids, 1),
        num=torch.cat(nums, 1),
        move_next=torch.cat([torch.zeros(b, 11, dtype=torch.long, device=dev),
                             m_next.long() % MOVE_BUCKETS,
                             torch.zeros(b, PRIV_T - 19, dtype=torch.long, device=dev)], 1),
        rooms=r.u8(P_POINTS, 96).long().clamp(0, ROOM_TYPES - 1),
        next_map=r.u8(P_NEXT + 4, 96, 5, 2).long().clamp(0, ROOM_TYPES - 1),
    )


# ---- A compact copy of the tokens (the learner's cache) ----

TOK_NUM = 11
"""Numeric columns a token past STATE can fill (the map's 11; the rest stay zero)."""
TOK_NUM_FULL = 12
"""With the full view: a pile token's 12."""
FULL_SLOTS = 20
"""Tokens per row that carry full-view extras (`hcat`, `hmove`, `hnum`): the creatures (8, in a
fight) or the map rows (`GRID_ROWS`, outside one). `pack` keeps only theirs, with positions."""
PRIV_NUM = 3
"""Numeric columns a privileged token can fill."""
_MOVE_AT = 11
"""The move tokens' first position in the privileged tokens (`move_next` is zero elsewhere)."""


# RLN29: the content stats (`sts2_rl::content_stats`) by ID row, and the token's upgrade flag.
STATS_UP_COL = {"hand": 0, "choice": 0, "deck": 0, "reward_card": 0, "select": 0,
                "bundle_card": 0, "shop_card": 0, "reward": 2, "pile": 0}
"""Per token kind with card idents, the `num` column holding the card's upgrade flag (raw 0/1;
a card reward's `rup`). Other kinds read the base row (relics, potions: one row)."""
_UP_COLS = {}


def token_up(kind, num):
    """(B, T) long: 1 where a token is an upgraded card (`STATS_UP_COL`), else 0."""
    key = str(kind.device)
    if key not in _UP_COLS:
        col = torch.zeros(len(K), dtype=torch.long)
        has = torch.zeros(len(K), dtype=torch.bool)
        for name, c in STATS_UP_COL.items():
            col[K[name]], has[K[name]] = c, True
        _UP_COLS[key] = (col.to(kind.device), has.to(kind.device))
    col, has = _UP_COLS[key]
    up = torch.gather(num, -1, col[kind].unsqueeze(-1)).squeeze(-1) > 0.5
    return (up & has[kind]).long()


def stats_table(lib=None, columns=False):
    """The content stats by ID row: a float32 (2, `ID_ROWS`, C) tensor (base rows, upgraded
    rows; relics and potions the same in both), the magnitude columns as `symlog`, the flags and
    the keyword deltas as they are. With `columns`, also the column names. Needs an ABI-11
    library (`sts2.ffi.content_stats`)."""
    from .. import ffi

    cols, t = ffi.content_stats(lib or ffi.load())
    out = torch.zeros(2, ID_ROWS, len(cols))
    for name, rows in (("cards", (t["cards"], t["cards_up"])),
                       ("relics", (t["relics"], t["relics"])),
                       ("potions", (t["potions"], t["potions"]))):
        cap = {"cards": 1024, "relics": 512, "potions": 128}[name]
        for up, tab in enumerate(rows):
            n = len(tab)
            assert n <= cap, f"{name}: {n} vocab ids, the ID table holds {cap}"
            out[up, _OFF[name]:_OFF[name] + n] = torch.from_numpy(tab[:n].astype("float32"))
    mag = torch.tensor([m for _, m in cols])
    out = torch.where(mag, symlog(out), out)
    return (out, [c for c, _ in cols]) if columns else out


def packed_shapes(num_dtype, v2=False, full=False, events=False):
    """{name: (per-row shape, dtype)} of `pack`'s output (with the v2 extras when `v2`, the
    full view's when `full` and the event section's when `events`)."""
    t = t_max(full)
    pos = torch.int16 if t > 128 else torch.int8  # token positions (`ent`, `tgt`)
    extra = dict(
        xpos=((FULL_SLOTS,), torch.int16), hcat=((FULL_SLOTS, HCAT), torch.int16),
        hmove=((FULL_SLOTS, HIST), torch.int16), hnum=((FULL_SLOTS, HNUM), num_dtype),
    ) if full else {}
    extra |= dict(
        ev_slot=((t,), torch.int8), evec=((8, EV_COLS), num_dtype),
        eids=((8, EV_IDS), torch.int16), ench=((8,), torch.int8),
    ) if events else {}
    extra |= dict(
        link=((t,), torch.int8), xcat=((t, 3), torch.int8), xnum=((t, XNUM), num_dtype),
        znum=((t, ZNUM), num_dtype), anum=((COMBAT_ACTIONS, ANUM), num_dtype),
        y_valid=((MAPNODES,), torch.bool), y_cat=((MAPNODES,), torch.int8),
        y_num=((MAPNODES, XNUM), num_dtype),
    ) if v2 else {}
    return extra | dict(
        valid=((t,), torch.bool), kind=((t,), torch.int8), ident=((t,), torch.int16),
        cat=((t,), torch.int16), owner=((t,), torch.int8),
        num0=((NUM,), num_dtype), num=((t - 1, TOK_NUM_FULL if full else TOK_NUM), num_dtype),
        bag_id=((BAG,), torch.int16), bag_up=((BAG,), torch.uint8),
        bag_group=((BAG,), torch.int8), ent=((ACTIONS_ALL,), pos),
        tgt=((ACTIONS_ALL,), pos), width=((), torch.int16), combat=((), torch.bool),
        p_ident=((PRIV_T,), torch.int16), p_valid=((PRIV_T,), torch.bool),
        p_num=((PRIV_T, PRIV_NUM), num_dtype), p_move_next=((8,), torch.int16),
        p_rooms=((96,), torch.uint8), p_next_map=((96,), torch.uint8),
    )


def pack(tok, ptok, num_dtype=torch.float32, most=None):
    """`encode`'s and `encode_priv`'s tensors that the net reads, in narrow dtypes (every
    value fits; the zero numeric columns are left out). `num_dtype` bfloat16 is exact for a net
    run under bf16 autocast, whose stems cast their input to bf16 anyway. `unpack` inverts it.
    The v2 extras and the full view's come along when `tok` has them. `most` (RLN44): a list
    that gets the full view's most extras per row as a device scalar instead of the check (a
    device sync); its caller checks it against `FULL_SLOTS` (`check_full_slots`) later."""
    pos = torch.int16 if tok["valid"].shape[1] > 128 else torch.int8  # `ent`, `tgt`
    full = "hcat" in tok
    extra = _pack_full(tok, num_dtype, most) if full else {}
    extra |= dict(
        ev_slot=tok["ev_slot"].to(torch.int8), evec=tok["evec"].to(num_dtype),
        eids=tok["eids"].to(torch.int16), ench=tok["ench"].to(torch.int8),
    ) if "evec" in tok else {}
    extra |= dict(
        link=tok["link"].to(torch.int8), xcat=tok["xcat"].to(torch.int8),
        xnum=tok["xnum"].to(num_dtype), znum=tok["znum"].to(num_dtype),
        anum=tok["anum"][:, :COMBAT_ACTIONS].to(num_dtype),
        y_valid=tok["y_valid"], y_cat=tok["y_cat"].to(torch.int8),
        y_num=tok["y_num"].to(num_dtype),
    ) if "xnum" in tok else {}
    return extra | dict(
        valid=tok["valid"], kind=tok["kind"].to(torch.int8), ident=tok["ident"].to(torch.int16),
        cat=tok["cat"].to(torch.int16), owner=tok["owner"].to(torch.int8),
        num0=tok["num"][:, 0].to(num_dtype),
        num=tok["num"][:, 1:, :TOK_NUM_FULL if full else TOK_NUM].to(num_dtype),
        bag_id=tok["bag_id"].to(torch.int16), bag_up=tok["bag_up"].to(torch.uint8),
        bag_group=tok["bag_group"].to(torch.int8), ent=tok["ent"].to(pos),
        tgt=tok["tgt"].to(pos), width=tok["width"].to(torch.int16),
        combat=tok["combat"],
        p_ident=ptok["ident"].to(torch.int16), p_valid=ptok["valid"],
        p_num=ptok["num"][:, :, :PRIV_NUM].to(num_dtype),
        p_move_next=ptok["move_next"][:, _MOVE_AT:_MOVE_AT + 8].to(torch.int16),
        p_rooms=ptok["rooms"].to(torch.uint8), p_next_map=ptok["next_map"].to(torch.uint8),
    )


def check_full_slots(most):
    """`pack`'s deferred check: `most` its list of device scalars."""
    if most and int(torch.stack(most).max()) > FULL_SLOTS:
        raise ValueError(f"more than {FULL_SLOTS} tokens with full-view extras")


def _pack_full(tok, num_dtype, most=None):
    """The full view's extras of the (at most `FULL_SLOTS`) tokens that have any, in token
    order, and their positions (-1 none)."""
    has = tok["hcat"].ne(0).any(-1) | tok["hmove"].ne(0).any(-1) | tok["hnum"].ne(0).any(-1)
    if most is not None:
        most.append(has.sum(1).max())
    elif int(has.sum(1).max()) > FULL_SLOTS:
        raise ValueError(f"more than {FULL_SLOTS} tokens with full-view extras")
    order = torch.argsort((~has).to(torch.int8), dim=1, stable=True)[:, :FULL_SLOTS]
    keep = torch.gather(has, 1, order)
    g = lambda x: torch.gather(x, 1, order.unsqueeze(-1).expand(-1, -1, x.shape[-1]))  # noqa: E731
    return dict(xpos=torch.where(keep, order, -1).to(torch.int16),
                hcat=g(tok["hcat"]).to(torch.int16), hmove=g(tok["hmove"]).to(torch.int16),
                hnum=g(tok["hnum"]).to(num_dtype))


def _unpack_full(p, t):
    """`_pack_full`'s output -> the (B, `t`, ...) extras."""
    pos = p["xpos"].long()
    b = pos.shape[0]
    at = torch.where(pos >= 0, pos, t)  # none -> a dropped column
    out = {}
    for k, dtype in (("hcat", torch.long), ("hmove", torch.long), ("hnum", p["hnum"].dtype)):
        x = p[k].to(dtype)
        full = torch.zeros(b, t + 1, x.shape[-1], dtype=dtype, device=x.device)
        full.scatter_(1, at.unsqueeze(-1).expand(-1, -1, x.shape[-1]), x)
        out[k] = full[:, :t]
    return out


_PRIV_KIND = {}


def _priv_kind(device):
    key = str(device)
    if key not in _PRIV_KIND:
        sizes = [("priv_state", 1), ("draw", 10), ("move", 8), ("rooms", 1), ("normal", 8),
                 ("elite", 4), ("event", 8), ("next_boss", 2), ("next_map", 1)]
        _PRIV_KIND[key] = torch.cat([torch.full((n,), PK[k], dtype=torch.long)
                                     for k, n in sizes]).to(device)
    return _PRIV_KIND[key]


def unpack(packed, masks, width, discard=False):
    """Packed rows (a dict of tensors indexed to a batch) and their mask words -> the `(tok,
    ptok)` dicts the net reads, with the tokens cut to `width` (a host int, at least the rows'
    longest; `tok["t"]` carries it, so the net does not sync on it). The numeric columns come
    back in the packed dtype."""
    p = packed
    b, t = masks.shape[0], width
    dev = masks.device
    num = torch.zeros(b, t, NUM, dtype=p["num"].dtype, device=dev)
    num[:, 0] = p["num0"]
    num[:, 1:, :p["num"].shape[-1]] = p["num"][:, :t - 1]
    table = _action_tables(dev)
    tok = dict(
        valid=p["valid"][:, :t], kind=p["kind"][:, :t].long(), ident=p["ident"][:, :t].long(),
        cat=p["cat"][:, :t].long(), owner=p["owner"][:, :t].long(), num=num,
        bag_id=p["bag_id"].long(), bag_up=p["bag_up"].float(), bag_group=p["bag_group"].long(),
        combat=p["combat"], legal=mask_bits(masks, discard), width=p["width"].long(),
        ent=p["ent"].long(), tgt=p["tgt"].long(), akind=table[:, 0], asub=table[:, 2], t=t,
    )
    if "xnum" in p:
        tok.update(link=p["link"][:, :t].long(), xcat=p["xcat"][:, :t].long(),
                   xnum=p["xnum"][:, :t], znum=p["znum"][:, :t], y_valid=p["y_valid"],
                   y_cat=p["y_cat"].long(),
                   y_num=p["y_num"],
                   anum=torch.cat([p["anum"], torch.zeros(b, tok["legal"].shape[1] - COMBAT_ACTIONS, ANUM,
                                                          dtype=p["anum"].dtype, device=dev)],
                                  1))
    if "hcat" in p:
        tok.update(_unpack_full(p, t))
    if "evec" in p:
        tok.update(ev_slot=p["ev_slot"][:, :t].long(), evec=p["evec"], eids=p["eids"].long(),
                   ench=p["ench"].long())
    pnum = torch.zeros(b, PRIV_T, NUM, dtype=p["p_num"].dtype, device=dev)
    pnum[:, :, :PRIV_NUM] = p["p_num"]
    move_next = torch.zeros(b, PRIV_T, dtype=torch.long, device=dev)
    move_next[:, _MOVE_AT:_MOVE_AT + 8] = p["p_move_next"].long()
    ptok = dict(kind=_priv_kind(dev).expand(b, -1), ident=p["p_ident"].long(),
                valid=p["p_valid"], num=pnum, move_next=move_next,
                rooms=p["p_rooms"].long(), next_map=p["p_next_map"].long())
    return tok, ptok
