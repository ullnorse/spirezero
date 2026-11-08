"""Day 1: a random agent on the real env.

Run:  .venv/bin/python learn/rl/exercises/week1/day1_loop.py
Test: .venv/bin/python -m pytest learn/rl/exercises/week1/test_day1.py

Fill in every TODO. Each function's docstring is its contract; the tests check it.
"""

import pathlib
import sys
import time

ROOT = pathlib.Path(__file__).resolve()
while not (ROOT / "Cargo.toml").exists():
    ROOT = ROOT.parent
sys.path[:0] = [str(ROOT / "python"), str(ROOT / "learn/rl/exercises")]

import numpy as np  # noqa: E402

from common import make_envs  # noqa: E402


def legal_indices(mask_row):
    """The action indices whose bit is set in one env's mask (3 uint64 words, little-endian
    bit order: index i is bit i % 64 of word i // 64). Returns an int array, ascending.

    Hint: np.unpackbits(mask_row.view(np.uint8), bitorder="little") gives one byte per bit.
    """
    raise NotImplementedError  # TODO


def random_actions(legal, rng):
    """One uniformly random legal action per env. `legal` is the n × 140 bool matrix
    (`envs.legal()`); `rng` is a `numpy.random.Generator`.

    Hint: give every legal cell a random score and every illegal cell -1, then argmax per row.
    """
    raise NotImplementedError  # TODO


def run_random(n_envs=64, n_steps=5000, seed=0):
    """Steps `n_envs` fights `n_steps` times with random legal actions. Returns a dict with
    `episodes` (fights that ended), `wins`, `win_rate`, `steps_per_s`.

    `envs = make_envs(n_envs, seed=seed)`. After `envs.step(actions)`, `envs.done[i]` is 1 if
    env i's fight ended on that step and `envs.won[i]` is 1 if it was won; the env has already
    been reset, so `envs.obs[i]` and `envs.legal()[i]` belong to the next fight. Call
    `envs.close()` when done.
    """
    raise NotImplementedError  # TODO


if __name__ == "__main__":
    r = run_random()
    print(f"episodes {r['episodes']}  wins {r['wins']}  win rate {r['win_rate']:.3f}  "
          f"{r['steps_per_s']:,.0f} steps/s")
