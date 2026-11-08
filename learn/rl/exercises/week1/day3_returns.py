"""Day 3: rewards, returns, discounting, on the real env.

Run:  .venv/bin/python learn/rl/exercises/week1/day3_returns.py
Test: .venv/bin/python -m pytest learn/rl/exercises/week1/test_day3.py
"""

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve()
while not (ROOT / "Cargo.toml").exists():
    ROOT = ROOT.parent
sys.path[:0] = [str(ROOT / "python"), str(ROOT / "learn/rl/exercises")]

import numpy as np  # noqa: E402

from common import make_envs  # noqa: E402
from week1.day2_observe import cards_first  # noqa: E402  (your day 2)


def returns(rewards, gamma=1.0):
    """G_t for every step of one episode: G_t = r_t + gamma * G_{t+1}, with G_T = r_T for the
    last step. One backwards pass. Returns a list the length of `rewards`."""
    raise NotImplementedError  # TODO


def terminal_rewards(done, won):
    """SpireZero's reward for one step of a batch: +1 where the fight was won, -1 where it
    ended any other way, 0 elsewhere. `done` and `won` are uint8 arrays of length n; return a
    float array."""
    raise NotImplementedError  # TODO


def episode_returns(rewards, done, gamma=1.0):
    """Returns of every episode in a batch rollout. `rewards` and `done` are T × n (step,
    env). An env's episode runs from the step after its last `done` to its next `done`;
    unfinished tails are dropped. Returns the list of G_0 values, in order of ending."""
    raise NotImplementedError  # TODO


def hp_shaped(obs_before, obs_after, done, won, scale=0.01):
    """A shaped reward: the terminal ±1 plus `-scale` per HP point lost this step (HP is the
    i16 at bytes 0-1). On a step that ended the fight the HP term is 0, because `obs_after` is
    already the next fight's first observation."""
    raise NotImplementedError  # TODO


def collect(policy, n_envs=64, n_steps=2000, seed=0):
    """Rolls `policy(legal) -> actions` out and returns (rewards, done, lengths): rewards and
    done as T × n arrays, lengths the per-episode step counts in the order the episodes
    ended."""
    raise NotImplementedError  # TODO


if __name__ == "__main__":
    print("returns of [0, 0, 0, 1], gamma 0.9:", [round(g, 3) for g in returns([0, 0, 0, 1], 0.9)])
    rng = np.random.default_rng(0)
    rand = lambda legal: np.where(legal, rng.random(legal.shape), -1.0).argmax(axis=1)  # noqa: E731
    for name, pol in (("random", rand), ("cards first", cards_first)):
        rewards, done, lengths = collect(pol)
        for gamma in (1.0, 0.99, 0.9):
            g = episode_returns(rewards, done, gamma)
            print(f"{name:12s} gamma {gamma:<5} mean return {np.mean(g):+.3f} over {len(g)} fights, "
                  f"mean length {np.mean(lengths):.1f} steps")
