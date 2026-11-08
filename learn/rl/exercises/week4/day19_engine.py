"""Day 19: the engine side. What the mask and the buffers promise, checked from Python over
thousands of steps; what an illegal action does; steps per second against threads to size a
batch.

Run:  .venv/bin/python learn/rl/exercises/week4/day19_engine.py
"""

import pathlib
import sys
import time

ROOT = pathlib.Path(__file__).resolve()
while not (ROOT / "Cargo.toml").exists():
    ROOT = ROOT.parent
sys.path[:0] = [str(ROOT / "python"), str(ROOT / "learn/rl/exercises")]

import numpy as np  # noqa: E402

from common import ACTIONS, DONE, END_TURN, HAND_SLOTS, PICK_BASE, POTION_BASE, POTION_SLOTS, TARGET_SLOTS, make_envs  # noqa: E402

MAX_EPISODE_ACTIONS = 10_000


def check_header(obs):
    """Violations of the header's promises over a batch (n × 972 uint8), as a list of
    strings, empty if none: 0 ≤ hp ≤ max_hp, block ≥ 0, energy ≥ 0, max_energy ≥ 0 (i16 at
    0, 2, 4, 6, 8), hand ≤ 10 (byte 16), status in 0..4 (byte 23)."""
    raise NotImplementedError  # TODO


def check_mask(obs, legal):
    """Violations of the mask's promises over a batch, as strings: every env has at least one
    legal action (the env resets a finished fight before you see it); a legal play index
    names a hand slot below the hand size (byte 16); a legal potion index names a belt slot
    below 6; at most one of End turn and Done is legal; no bit at or past ACTIONS (the mask
    has exactly ACTIONS columns)."""
    raise NotImplementedError  # TODO


def check_stats(stats, n, done, won):
    """Violations of `envs.stats()` after a step of `n` envs, as strings: actions == n, ended
    == done.sum(), won == won.sum() and ≤ ended, capped ≤ ended, reset == ended (the default
    starts never run out)."""
    raise NotImplementedError  # TODO


def illegal_is_rejected(envs):
    """Steps `envs` with an illegal index in env 0 (the lowest index whose bit is clear) and
    legal ones elsewhere. Returns (error message or None, observation unchanged: bool)."""
    raise NotImplementedError  # TODO


def audit(policy, n_envs=64, n_steps=2000, seed=0, threads=1):
    """Runs `policy(legal) -> actions` for `n_steps` steps and checks the header, the mask and
    the stats after every step. Returns (violations: list of (step, message), fights ended,
    fights capped)."""
    raise NotImplementedError  # TODO


def bench(threads_list=(1, 2, 4, 8), n_envs=256, n_steps=500, seed=0):
    """Env steps per second of random legal play for each thread count, as {threads:
    steps_per_second}, with the same actions for every count (a fresh batch and policy per
    count)."""
    raise NotImplementedError  # TODO


if __name__ == "__main__":
    from week1.day2_observe import cards_first

    violations, ended, capped = audit(cards_first, n_steps=2000)
    print(f"audit: {len(violations)} violations over 2,000 steps × 64 envs, {ended} fights, {capped} capped")
    for t, m in violations[:5]:
        print(f"  step {t}: {m}")
    envs = make_envs(4, seed=1)
    msg, unchanged = illegal_is_rejected(envs)
    envs.close()
    print(f"illegal action: {msg!r}, observation unchanged: {unchanged}")
    for threads, sps in bench().items():
        print(f"threads {threads}: {sps:,.0f} steps/s")
