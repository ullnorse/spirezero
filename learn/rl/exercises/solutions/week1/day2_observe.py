"""Day 2: decode the observation and the action index; a first scripted policy. Reference."""

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve()
while not (ROOT / "Cargo.toml").exists():
    ROOT = ROOT.parent
sys.path[:0] = [str(ROOT / "python"), str(ROOT / "learn/rl/exercises")]

import numpy as np  # noqa: E402

from common import (CARD_BYTES, CARDS_AT, DONE, END_TURN, HAND_AT, INTENTS, INTENTS_AT, INTENT_BYTES, PICK_BASE,  # noqa: E402
                    POTION_BASE, TARGET_SLOTS, make_envs, vocab)

STATUS = ("player turn", "decision", "won", "lost", "unsupported")


def header(obs_row):
    """The 24-byte header as a dict: hp, max_hp, block, energy, max_energy (i16 at 0, 2, 4, 6,
    8), turn (u16 at 12), hand, draw, discard (u8 at 16, 17, 18), status (u8 at 22)."""
    b = np.asarray(obs_row, dtype=np.uint8)
    i16 = b[:12].copy().view("<i2")
    turn = int(b[12:14].copy().view("<u2")[0])
    return dict(hp=int(i16[0]), max_hp=int(i16[1]), block=int(i16[2]), energy=int(i16[3]),
                max_energy=int(i16[4]), turn=turn, hand=int(b[16]), draw=int(b[17]),
                discard=int(b[18]), status=int(b[22]))


def hand_names(obs_row, cards=None):
    """The cards in hand, as names, in hand order. The hand is `hand` (header byte 16) card
    ids (u16) at HAND_AT. A card id is a row of the cards table: its kind is the u16 at
    CARDS_AT + id * CARD_BYTES, and the kind's name is `vocab("cards")[kind]`."""
    cards = cards if cards is not None else vocab("cards")
    b = np.asarray(obs_row, dtype=np.uint8)
    n = int(b[16])
    ids = b[HAND_AT:HAND_AT + 2 * n].copy().view("<u2")
    kinds = [int(b[CARDS_AT + i * CARD_BYTES:CARDS_AT + i * CARD_BYTES + 2].copy().view("<u2")[0]) for i in ids]
    return [cards[k] for k in kinds]


def intents(obs_row):
    """The enemies' intents as (kind, damage, hits) for every filled slot. A slot is 5 bytes
    at INTENTS_AT: kind + 1 (u8; 0 means empty), damage per hit (i16), hits (u8), status cards
    (u8). Kinds: 0 attack, 1 buff, 2 debuff, 3 strong debuff, 4 defend, 5 escape, 6 heal."""
    b = np.asarray(obs_row, dtype=np.uint8)
    out = []
    for i in range(INTENTS):
        at = INTENTS_AT + i * INTENT_BYTES
        if b[at] == 0:
            continue
        damage = int(b[at + 1:at + 3].copy().view("<i2")[0])
        out.append((int(b[at]) - 1, damage, int(b[at + 3])))
    return out


def decode_action(index):
    """What an action index means: ("play", hand_slot, target), ("potion", slot, target),
    ("end",), ("pick", option) or ("done",). Targets: 0 is none/self, 1-7 enemies."""
    if index < POTION_BASE:
        return ("play", int(index) // TARGET_SLOTS, int(index) % TARGET_SLOTS)
    if index < END_TURN:
        i = index - POTION_BASE
        return ("potion", int(i) // TARGET_SLOTS, int(i) % TARGET_SLOTS)
    if index == END_TURN:
        return ("end",)
    if index < DONE:
        return ("pick", int(index) - PICK_BASE)
    return ("done",)


def cards_first(legal):
    """One action per env: the lowest legal play or potion index if there is one, else the
    lowest legal index (End turn, a pick, or Done)."""
    cards = legal.copy()
    cards[:, END_TURN:] = False
    has_card = cards.any(axis=1)
    return np.where(has_card, cards.argmax(axis=1), legal.argmax(axis=1))


def win_rate(policy, n_envs=64, n_steps=3000, seed=0):
    """Win rate of `policy(legal) -> actions` over the fights that end in `n_steps` steps."""
    envs = make_envs(n_envs, seed=seed)
    episodes = wins = 0
    for _ in range(n_steps):
        envs.step(policy(envs.legal()))
        episodes += int(envs.done.sum())
        wins += int(envs.won.sum())
    envs.close()
    return wins / max(episodes, 1), episodes


if __name__ == "__main__":
    envs = make_envs(4, seed=1)
    cards = vocab("cards")
    for i in range(envs.n):
        h = header(envs.obs[i])
        print(f"env {i}: HP {h['hp']}/{h['max_hp']} block {h['block']} energy {h['energy']} "
              f"turn {h['turn']} hand {hand_names(envs.obs[i], cards)} intents {intents(envs.obs[i])}")
        print("   legal:", [decode_action(a) for a in np.flatnonzero(envs.legal()[i])])
    envs.close()
    rng = np.random.default_rng(0)
    rand = lambda legal: np.where(legal, rng.random(legal.shape), -1.0).argmax(axis=1)  # noqa: E731
    for name, pol in (("random", rand), ("cards first", cards_first)):
        wr, n = win_rate(pol)
        print(f"{name:12s} win rate {wr:.3f} over {n} fights")
