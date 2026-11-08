"""Shared helpers for the exercises: repo paths, the real env, vocabularies."""

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve()
while not (ROOT / "Cargo.toml").exists():
    ROOT = ROOT.parent

for p in (ROOT / "python", ROOT / "learn" / "rl" / "exercises"):
    if str(p) not in sys.path:
        sys.path.insert(0, str(p))

VOCAB_DIR = ROOT / "crates" / "sts2-rl" / "vocab"


def vocab(name):
    """The id → name list of `crates/sts2-rl/vocab/<name>.txt` (line number = id)."""
    return (VOCAB_DIR / f"{name}.txt").read_text().splitlines()


def make_envs(n, seed=0, threads=1):
    """`sts2.Sts2Envs(n)` on the default Act 1 starts. Fails with the build command if the
    FFI library is missing."""
    import sts2

    return sts2.Sts2Envs(n, seed=seed, threads=threads)


# Offsets inside the 972-byte combat observation (crates/sts2-rl/src/obs.rs).
HEADER = 24
CREATURES_AT = 24
CREATURE_BYTES = 60
CREATURES = 8
POWERS = 12
CARDS_AT = 504
CARD_BYTES = 6
CARDS = 64
HAND_AT = 888
HAND = 10
INTENTS_AT = 908
INTENT_BYTES = 5
INTENTS = 8
CHOICE_AT = 948
OBS_BYTES = 972

# The 140-index action space (crates/sts2-rl/src/action.rs).
HAND_SLOTS = 10
TARGET_SLOTS = 8
POTION_SLOTS = 6
PLAY_BASE = 0
POTION_BASE = 80
END_TURN = 128
PICK_BASE = 129
DONE = 139
ACTIONS = 140
