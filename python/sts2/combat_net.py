"""The combat value/policy net (RLB11): features of the combat observation, the net, and its
export for the Rust search (`crates/sts2-diff/src/honest/value.rs` mirrors `features` exactly).

The input is the observation the leak test covers (`crates/sts2-rl/src/obs.rs`: the dynamic
part, then the static part), so the net sees only what the player sees. `features` turns a
batch of raw bytes into one float vector per row: scaled scalars, position-specific embeddings
(hand slots, creatures, intents, choice options, potions, the encounter) concatenated, and
pooled embeddings (cards by pile, powers, relics) summed. Then a 2-layer ReLU MLP with four
heads: the policy logits (140), the win logit, the HP logit (the HP fraction kept, 0 for a
loss) and nothing else; the PPO critic is `P(win) + HP_BONUS * hp`.
"""

import struct

import numpy as np
import torch
import torch.nn as nn

# Observation layout (`obs.rs`).
HEADER = 24
CREATURES, CREATURE_BYTES, POWERS = 8, 60, 12
CREATURES_AT = HEADER
CARDS, CARD_BYTES = 64, 6
CARDS_AT = CREATURES_AT + CREATURES * CREATURE_BYTES
HAND = 10
HAND_AT = CARDS_AT + CARDS * CARD_BYTES
INTENTS, INTENT_BYTES = 8, 5
INTENTS_AT = HAND_AT + HAND * 2
OPTIONS = 10
CHOICE_AT = INTENTS_AT + INTENTS * INTENT_BYTES
OBS_BYTES = CHOICE_AT + OPTIONS * 2 + 4
DECK, RELICS, POTIONS = 64, 32, 6
STATIC_BYTES = 6 + DECK * 3 + RELICS * 2 + POTIONS * 2
RELICS_AT = OBS_BYTES + 6 + DECK * 3
POTIONS_AT = RELICS_AT + RELICS * 2
ACTIONS = 140

# Embedding tables: rows (ids at or past the last row share it) and widths.
TABLES = {
    "card": (1024, 32),
    "monster": (256, 16),
    "power_kind": (512, 16),
    "power_amount": (512, 16),
    "intent": (32, 8),
    "relic": (512, 16),
    "potion": (128, 16),
    "encounter": (128, 16),
}
TABLE_ORDER = tuple(TABLES)

HP_BONUS = 0.5
"""The critic's and the search's weight of the HP fraction kept against a win."""

HIDDEN = 512

DENSE_HEADER = 17
PER_CREATURE = 6 + 16 + 16 + 16
PER_HAND = 32 + 9
PER_INTENT = 8 + 4
PER_OPTION = 32 + 1
FEATURES = (DENSE_HEADER + CREATURES * PER_CREATURE + 4 * 32 + HAND * PER_HAND
            + INTENTS * PER_INTENT + OPTIONS * PER_OPTION + 3 + 8 + 16 + 16 + POTIONS * 16)


def _u16(raw, at):
    return raw[..., at] + raw[..., at + 1] * 256


def _i16(raw, at):
    v = _u16(raw, at)
    return v - (v >= 32768).to(v.dtype) * 65536


def _cols(raw, first, stride, count, offset):
    """Column indices `first + i * stride + offset` for i < count."""
    return [first + i * stride + offset for i in range(count)]


def _u16s(raw, first, stride, count, offset=0):
    lo = raw[:, _cols(raw, first, stride, count, offset)]
    hi = raw[:, _cols(raw, first, stride, count, offset + 1)]
    return lo + hi * 256


def _i16s(raw, first, stride, count, offset=0):
    v = _u16s(raw, first, stride, count, offset)
    return v - (v >= 32768).to(v.dtype) * 65536


def _u8s(raw, first, stride, count, offset=0):
    return raw[:, _cols(raw, first, stride, count, offset)]


class Net(nn.Module):
    def __init__(self, hidden=HIDDEN):
        super().__init__()
        self.emb = nn.ModuleDict({
            name: nn.Embedding(rows, width) for name, (rows, width) in TABLES.items()
        })
        for e in self.emb.values():
            nn.init.normal_(e.weight, std=0.3)
        self.l1 = nn.Linear(FEATURES, hidden)
        self.l2 = nn.Linear(hidden, hidden)
        self.policy = nn.Linear(hidden, ACTIONS)
        self.win = nn.Linear(hidden, 1)
        self.hp = nn.Linear(hidden, 1)
        nn.init.orthogonal_(self.l1.weight, 2 ** 0.5)
        nn.init.orthogonal_(self.l2.weight, 2 ** 0.5)
        nn.init.orthogonal_(self.policy.weight, 0.01)
        nn.init.zeros_(self.policy.bias)

    def _e(self, name, ids):
        rows = TABLES[name][0]
        return self.emb[name](ids.clamp(0, rows - 1))

    def features(self, raw):
        """`raw` (B × (OBS_BYTES + STATIC_BYTES), uint8) as B × FEATURES floats."""
        raw = raw.long()
        b = raw.shape[0]
        f = torch.float32
        hp, max_hp = _i16(raw, 0).to(f), _i16(raw, 2).to(f)
        status = raw[:, 22]
        out = [torch.stack([
            hp / 100, max_hp / 100, hp / max_hp.clamp(min=1), _i16(raw, 4).to(f) / 50,
            _i16(raw, 6).to(f) / 5, _i16(raw, 8).to(f) / 5, _i16(raw, 10).to(f) / 10,
            _u16(raw, 12).clamp(max=30).to(f) / 10, _u16(raw, 14).clamp(max=30).to(f) / 10,
            raw[:, 16].to(f) / 10, raw[:, 17].to(f) / 30, raw[:, 18].to(f) / 30,
            raw[:, 19].to(f) / 30, raw[:, 20].to(f) / 5, raw[:, 21].to(f) / 5,
            (status == 0).to(f), (status == 1).to(f),
        ], dim=1)]
        # Creatures.
        c_hp = _i16s(raw, CREATURES_AT, CREATURE_BYTES, CREATURES, 6).to(f)
        c_max = _i16s(raw, CREATURES_AT, CREATURE_BYTES, CREATURES, 8).to(f)
        dense = torch.stack([
            _u8s(raw, CREATURES_AT, CREATURE_BYTES, CREATURES, 0).to(f),
            _u8s(raw, CREATURES_AT, CREATURE_BYTES, CREATURES, 1).to(f),
            c_hp / 100, c_max / 100, c_hp / c_max.clamp(min=1),
            _i16s(raw, CREATURES_AT, CREATURE_BYTES, CREATURES, 10).to(f) / 50,
        ], dim=2)  # B × 8 × 6
        monster = self._e("monster", _u16s(raw, CREATURES_AT, CREATURE_BYTES, CREATURES, 4))
        kinds = torch.stack([
            _u16s(raw, CREATURES_AT + 12 + 4 * p, CREATURE_BYTES, CREATURES) for p in range(POWERS)
        ], dim=2)  # B × 8 × 12
        amounts = torch.stack([
            _i16s(raw, CREATURES_AT + 14 + 4 * p, CREATURE_BYTES, CREATURES)
            for p in range(POWERS)
        ], dim=2).to(f).clamp(-50, 50) / 10
        present = (kinds != 0).to(f).unsqueeze(-1)
        pk = (self._e("power_kind", kinds) * present).sum(2)
        pa = (self._e("power_amount", kinds) * amounts.unsqueeze(-1) * present).sum(2)
        out.append(torch.cat([dense, monster, pk, pa], dim=2).reshape(b, -1))
        # Cards by pile.
        kind = _u16s(raw, CARDS_AT, CARD_BYTES, CARDS)
        upgraded = _u8s(raw, CARDS_AT, CARD_BYTES, CARDS, 2).clamp(max=1)
        token = kind * 2 + upgraded
        pile = _u8s(raw, CARDS_AT, CARD_BYTES, CARDS, 3)
        card = self._e("card", token)  # B × 64 × 32
        valid = (kind != 0).to(f)
        for p in (1, 2, 3, 4):
            w = (valid * (pile == p).to(f)).unsqueeze(-1) / 8
            out.append((card * w).sum(1))
        # The hand, by slot.
        hand_ids = _u16s(raw, HAND_AT, 2, HAND)
        hand_n = raw[:, 16].unsqueeze(1)
        slot = torch.arange(HAND, device=raw.device).unsqueeze(0)
        here = ((slot < hand_n) & (hand_ids < CARDS)).to(f)
        idx = hand_ids.clamp(max=CARDS - 1)
        h_card = torch.gather(card, 1, idx.unsqueeze(-1).expand(-1, -1, card.shape[2]))
        costs = torch.stack([
            torch.gather(_u8s(raw, CARDS_AT, CARD_BYTES, CARDS, o), 1, idx) for o in (2, 4, 5)
        ], dim=2)  # upgrade, cost (i8 raw), flags
        cost = costs[..., 1]
        cost = (cost - (cost >= 128).long() * 256).to(f)
        flags = costs[..., 2]
        bits = torch.stack([((flags >> k) & 1).to(f) for k in range(6)], dim=2)
        hand = torch.cat([
            h_card, (cost / 5).unsqueeze(-1), costs[..., 0].clamp(max=1).to(f).unsqueeze(-1),
            bits, torch.ones_like(cost).unsqueeze(-1),
        ], dim=2) * here.unsqueeze(-1)
        out.append(hand.reshape(b, -1))
        # Intents.
        ik = _u8s(raw, INTENTS_AT, INTENT_BYTES, INTENTS)
        dmg = _i16s(raw, INTENTS_AT, INTENT_BYTES, INTENTS, 1).to(f)
        hits = _u8s(raw, INTENTS_AT, INTENT_BYTES, INTENTS, 3).to(f)
        status_cards = _u8s(raw, INTENTS_AT, INTENT_BYTES, INTENTS, 4).to(f)
        intents = torch.cat([
            self._e("intent", ik),
            torch.stack([dmg / 20, hits / 5, dmg * hits / 50, status_cards / 5], dim=2),
        ], dim=2)
        out.append(intents.reshape(b, -1))
        # A pending choice.
        opt = _u16s(raw, CHOICE_AT, 2, OPTIONS)
        cmax = raw[:, CHOICE_AT + OPTIONS * 2 + 1]
        first = (torch.arange(OPTIONS, device=raw.device) == 0).unsqueeze(0)
        o_here = (((opt != 0) | first) & (cmax > 0).unsqueeze(1) & (opt < CARDS)).to(f)
        o_card = torch.gather(card, 1, opt.clamp(max=CARDS - 1).unsqueeze(-1)
                              .expand(-1, -1, card.shape[2]))
        out.append((torch.cat([o_card, torch.ones_like(o_here).unsqueeze(-1)], dim=2)
                    * o_here.unsqueeze(-1)).reshape(b, -1))
        out.append(torch.stack([raw[:, CHOICE_AT + OPTIONS * 2 + k].to(f) / 5 for k in range(3)],
                               dim=1))
        # The static part.
        act = raw[:, OBS_BYTES + 2].clamp(max=3)
        room = raw[:, OBS_BYTES + 3].clamp(max=3)
        out.append(torch.cat([nn.functional.one_hot(act, 4).to(f),
                              nn.functional.one_hot(room, 4).to(f)], dim=1))
        out.append(self._e("encounter", _u16(raw, OBS_BYTES + 4)))
        relics = _u16s(raw, RELICS_AT, 2, RELICS)
        out.append((self._e("relic", relics) * (relics != 0).to(f).unsqueeze(-1)).sum(1) / 4)
        out.append(self._e("potion", _u16s(raw, POTIONS_AT, 2, POTIONS)).reshape(b, -1))
        x = torch.cat(out, dim=1)
        assert x.shape[1] == FEATURES, (x.shape, FEATURES)
        return x

    def forward(self, raw):
        h = torch.relu(self.l1(self.features(raw)))
        h = torch.relu(self.l2(h))
        return self.policy(h), self.win(h).squeeze(-1), self.hp(h).squeeze(-1)


def unpack_masks(masks, actions=ACTIONS):
    """`masks` (B × words, int64 on any device) as B × actions bools."""
    bits = (masks.unsqueeze(-1) >> torch.arange(64, device=masks.device)) & 1
    return bits.reshape(masks.shape[0], -1)[:, :actions].bool()


MAGIC = b"STS2VAL1"


def export(net, path, vocab_hash):
    """Writes `net` for the Rust search: MAGIC, then FEATURES, HIDDEN, ACTIONS (u32) and the
    vocabulary hash (u64), the tables in TABLE_ORDER (rows × width, row-major), then l1, l2,
    policy, win, hp as (weight `out × in` row-major, bias), all little-endian f32."""
    sd = {k: v.detach().float().cpu().numpy() for k, v in net.state_dict().items()}
    hidden = sd["l1.weight"].shape[0]
    with open(path, "wb") as f:
        f.write(MAGIC)
        f.write(struct.pack("<IIIQ", FEATURES, hidden, ACTIONS, vocab_hash))
        for name in TABLE_ORDER:
            f.write(sd[f"emb.{name}.weight"].astype("<f4").tobytes())
        for layer in ("l1", "l2", "policy", "win", "hp"):
            f.write(sd[f"{layer}.weight"].astype("<f4").tobytes())
            f.write(sd[f"{layer}.bias"].astype("<f4").tobytes())
