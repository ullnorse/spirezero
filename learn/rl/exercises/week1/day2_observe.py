"""Day 2: decode the observation and the action index; a first scripted policy.

Run:  .venv/bin/python learn/rl/exercises/week1/day2_observe.py
Test: .venv/bin/python -m pytest learn/rl/exercises/week1/test_day2.py
"""

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve()
while not (ROOT / "Cargo.toml").exists():
    ROOT = ROOT.parent
sys.path[:0] = [str(ROOT / "python"), str(ROOT / "learn/rl/exercises")]

import numpy as np  # noqa: E402

from common import (CARD_BYTES, CARDS_AT, DONE, END_TURN, HAND_AT, INTENTS, INTENTS_AT,  # noqa: E402,F401
                    INTENT_BYTES, PICK_BASE, POTION_BASE, TARGET_SLOTS, make_envs, vocab)

STATUS = ("player turn", "decision", "won", "lost", "unsupported")


def header(obs_row):
    """The 24-byte header as a dict: hp, max_hp, block, energy, max_energy (i16 at 0, 2, 4, 6,
    8), turn (u16 at 12), hand, draw, discard (u8 at 16, 17, 18), status (u8 at 22).

    Hint: obs_row[:12].copy().view("<i2") reads six little-endian i16 values.
    """
    raise NotImplementedError  # TODO


def hand_names(obs_row, cards=None):
    """The cards in hand, as names, in hand order. The hand is `hand` (header byte 16) card
    ids (u16) at HAND_AT. A card id is a row of the cards table: its kind is the u16 at
    CARDS_AT + id * CARD_BYTES, and the kind's name is `vocab("cards")[kind]`."""
    cards = cards if cards is not None else vocab("cards")
    raise NotImplementedError  # TODO


def intents(obs_row):
    """The enemies' intents as (kind, damage, hits) for every filled slot. A slot is 5 bytes
    at INTENTS_AT: kind + 1 (u8; 0 means empty), damage per hit (i16), hits (u8), status cards
    (u8). Kinds: 0 attack, 1 buff, 2 debuff, 3 strong debuff, 4 defend, 5 escape, 6 heal."""
    raise NotImplementedError  # TODO


def decode_action(index):
    """What an action index means: ("play", hand_slot, target), ("potion", slot, target),
    ("end",), ("pick", option) or ("done",). Targets: 0 is none/self, 1-7 enemies.
    Plays fill 0-79 (slot × 8 + target), potions 80-127, End turn is 128, picks 129-138,
    Done 139. Return plain ints."""
    raise NotImplementedError  # TODO


def cards_first(legal):
    """One action per env: the lowest legal play or potion index if there is one, else the
    lowest legal index (End turn, a pick, or Done). `legal` is n × 140 bools; return an int
    array of length n."""
    raise NotImplementedError  # TODO


def win_rate(policy, n_envs=64, n_steps=3000, seed=0):
    """Win rate of `policy(legal) -> actions` over the fights that end in `n_steps` steps, and
    the number of those fights, as a tuple."""
    raise NotImplementedError  # TODO


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
