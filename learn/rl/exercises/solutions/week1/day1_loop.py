"""Day 1: a random agent on the real env. Reference solution."""

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
    bit order: index i is bit i % 64 of word i // 64). Returns an int array, ascending."""
    bits = np.unpackbits(np.asarray(mask_row, dtype=np.uint64).view(np.uint8), bitorder="little")
    return np.flatnonzero(bits)


def random_actions(legal, rng):
    """One uniformly random legal action per env. `legal` is the n × 140 bool matrix
    (`envs.legal()`); `rng` is a `numpy.random.Generator`."""
    scores = np.where(legal, rng.random(legal.shape), -1.0)
    return scores.argmax(axis=1)


def run_random(n_envs=64, n_steps=5000, seed=0):
    """Steps `n_envs` fights `n_steps` times with random legal actions. Returns a dict with
    `episodes` (fights that ended), `wins`, `win_rate`, `steps_per_s`."""
    envs = make_envs(n_envs, seed=seed)
    rng = np.random.default_rng(seed)
    episodes = wins = 0
    t0 = time.perf_counter()
    for _ in range(n_steps):
        envs.step(random_actions(envs.legal(), rng))
        episodes += int(envs.done.sum())
        wins += int(envs.won.sum())
    dt = time.perf_counter() - t0
    envs.close()
    return dict(episodes=episodes, wins=wins, win_rate=wins / max(episodes, 1),
                steps_per_s=n_envs * n_steps / dt)


if __name__ == "__main__":
    r = run_random()
    print(f"episodes {r['episodes']}  wins {r['wins']}  win rate {r['win_rate']:.3f}  "
          f"{r['steps_per_s']:,.0f} steps/s")
