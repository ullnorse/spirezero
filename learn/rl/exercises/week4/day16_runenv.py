"""Day 16: the run env. A whole run as one episode, the run header, the indices after 140,
and run-level agents measured by the floor they reach on fixed seeds.

Run:  .venv/bin/python learn/rl/exercises/week4/day16_runenv.py
"""

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve()
while not (ROOT / "Cargo.toml").exists():
    ROOT = ROOT.parent
sys.path[:0] = [str(ROOT / "python"), str(ROOT / "learn/rl/exercises")]

import numpy as np  # noqa: E402

PHASES = ("ancient", "event", "map", "combat", "rewards", "select", "bundles", "rest", "treasure", "shop",
          "stub", "stopped", "game over", "victory")

# The run-level blocks of crates/sts2-rl/src/run_action.rs, after the 140 combat indices.
RUN_BASE = 140
ANCIENT_BASE = RUN_BASE
EVENT_BASE = ANCIENT_BASE + 8
NODE_BASE = EVENT_BASE + 8
CLAIM_BASE = NODE_BASE + 8
PICK_CARD_BASE = CLAIM_BASE + 8
ALTERNATIVE_BASE = PICK_CARD_BASE + 8 * 5
SKIP_BASE = ALTERNATIVE_BASE + 8 * 2
REST_BASE = SKIP_BASE + 8
USE_POTION_BASE = REST_BASE + 8
SELECT_BASE = USE_POTION_BASE + 6
CONFIRM_SELECT = SELECT_BASE + 80
CANCEL_SELECT = CONFIRM_SELECT + 1
TAKE_RELIC = CANCEL_SELECT + 1
BUY_CARD_BASE = TAKE_RELIC + 1
BUY_RELIC_BASE = BUY_CARD_BASE + 8
BUY_POTION_BASE = BUY_RELIC_BASE + 4
REMOVE_CARD = BUY_POTION_BASE + 4
BUNDLE_BASE = REMOVE_CARD + 1
PROCEED = BUNDLE_BASE + 4
RUN_ACTIONS = PROCEED + 1


def make_run_envs(n, seeds=(0, 1000), combat="greedy", character="Necrobinder", threads=1):
    """`sts2.Sts2RunEnvs(n, seed_range=seeds, cycle=True, combat=combat, ...)`: every env plays
    the seeds of the range in order and cycles, so two agents measured on the same range see
    the same runs."""
    import sts2

    return sts2.Sts2RunEnvs(n, seed_range=tuple(seeds), cycle=True, combat=combat, character=character, threads=threads)


def run_header(obs_row):
    """The run header as a dict: character, ascension, act_index, phase (u8 at 0-3; phase as
    its PHASES name), act (u16 at 4), floor_in_act, floor (u8 at 6, 7), hp, max_hp, gold (i16
    at 8, 10, 12), max_energy, belt (u8 at 14, 15), deck_size (u16 at 16), relics,
    removals (u8 at 18, 19), map_col, map_row (u8 at 26, 27; 255 before the first point),
    room_type (u8 at 28)."""
    raise NotImplementedError  # TODO


def decode_run_action(index):
    """What a run-space index means, as (kind, i): ("combat", index) below RUN_BASE, then
    ("ancient", i), ("event", i), ("node", column), ("claim", reward), ("pick_card", (reward,
    card)), ("alternative", (reward, a)), ("skip", reward), ("rest", i), ("use_potion", slot),
    ("select", i), ("confirm", 0), ("cancel", 0), ("take_relic", 0), ("buy_card", i),
    ("buy_relic", i), ("buy_potion", i), ("remove_card", 0), ("bundle", i), ("proceed", 0)."""
    raise NotImplementedError  # TODO


def random_run_policy(obs, legal, rng=np.random.default_rng(0)):
    """A uniform legal run-level action per env."""
    return np.where(legal, rng.random(legal.shape), -1.0).argmax(axis=1)


def lowest_policy(obs, legal):
    """The lowest legal index per env: first option, first column, claim before skip."""
    return legal.argmax(axis=1)


def prefer_policy(order):
    """`policy(obs, legal)` that plays the lowest legal index of the first block of `order`
    (a list of (base, slots) pairs) that has one, else the lowest legal index at all."""
    raise NotImplementedError  # TODO


def measure_floors(policy, n_runs=200, n_envs=64, seeds=(0, 1000), combat="greedy", threads=4, max_steps=5000):
    """Plays `policy(obs, legal) -> actions` on `make_run_envs` until `n_runs` runs have ended
    or `max_steps` steps were taken (a policy that loops, select then cancel for ever, would
    otherwise run until the 60,000-action cap). Returns (mean final floor, standard error,
    list of floors, {end name: count}) using `envs.floor`, `envs.end` and
    sts2.env.RUN_ENDS; the mean is nan with no finished run."""
    raise NotImplementedError  # TODO


if __name__ == "__main__":
    envs = make_run_envs(2)
    print(run_header(envs.obs[0]))
    legal = envs.legal()[0]
    print("legal now:", [decode_run_action(i) for i in np.flatnonzero(legal)])
    envs.close()
    for name, policy in (("random", random_run_policy), ("lowest index", lowest_policy),
                         ("claim, take the first card, rest", prefer_policy([(REST_BASE, 1), (CLAIM_BASE, 8), (PICK_CARD_BASE, 40), (PROCEED, 1)])),
                         ("skip every card", prefer_policy([(SKIP_BASE, 8), (PROCEED, 1), (CLAIM_BASE, 8)]))):
        mean, se, floors, ends = measure_floors(policy)
        print(f"{name:34s} floor {mean:5.2f} ± {se:.2f}  max {max(floors)}  ends {ends}")
