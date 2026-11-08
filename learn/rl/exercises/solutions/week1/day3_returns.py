"""Day 3: rewards, returns, discounting, on the real env. Reference solution."""

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve()
while not (ROOT / "Cargo.toml").exists():
    ROOT = ROOT.parent
sys.path[:0] = [str(ROOT / "python"), str(ROOT / "learn/rl/exercises")]

import numpy as np  # noqa: E402

from common import make_envs  # noqa: E402
from solutions.week1.day2_observe import cards_first, header  # noqa: E402


def returns(rewards, gamma=1.0):
    """G_t for every step of one episode: G_t = r_t + gamma * G_{t+1}, G_T = r_T.
    Computed backwards in one pass. Returns a list the length of `rewards`."""
    out = [0.0] * len(rewards)
    g = 0.0
    for t in range(len(rewards) - 1, -1, -1):
        g = rewards[t] + gamma * g
        out[t] = g
    return out


def terminal_rewards(done, won):
    """SpireZero's reward for one step of a batch: +1 where the fight was won, -1 where it
    ended any other way, 0 elsewhere. `done` and `won` are uint8 arrays of length n."""
    done = np.asarray(done, dtype=bool)
    return np.where(done, np.where(np.asarray(won, dtype=bool), 1.0, -1.0), 0.0)


def episode_returns(rewards, done, gamma=1.0):
    """Returns of every episode in a batch rollout. `rewards` and `done` are T × n (step,
    env). An env's episode runs from the step after its last `done` to its next `done`;
    unfinished tails are dropped. Returns the list of G_0 values, in order of ending."""
    rewards = np.asarray(rewards, dtype=float)
    done = np.asarray(done, dtype=bool)
    T, n = rewards.shape
    out = []
    start = [0] * n
    for t in range(T):
        for i in np.flatnonzero(done[t]):
            out.append(returns(rewards[start[i]:t + 1, i], gamma)[0])
            start[i] = t + 1
    return out


def hp_shaped(obs_before, obs_after, done, won, scale=0.01):
    """A shaped reward: the terminal ±1 plus `-scale` per HP point lost this step. On a step
    that ended the fight the HP term is 0 (the observation is already the next fight's)."""
    done = np.asarray(done, dtype=bool)
    hp_before = obs_before[:, 0:2].copy().view("<i2").ravel().astype(float)
    hp_after = obs_after[:, 0:2].copy().view("<i2").ravel().astype(float)
    loss = np.where(done, 0.0, np.maximum(0.0, hp_before - hp_after))
    return terminal_rewards(done, won) - scale * loss


def collect(policy, n_envs=64, n_steps=2000, seed=0):
    """Rolls `policy(legal) -> actions` out and returns (rewards, done) as T × n arrays plus
    the per-episode lengths, in the order the episodes ended."""
    envs = make_envs(n_envs, seed=seed)
    rewards = np.zeros((n_steps, n_envs))
    done = np.zeros((n_steps, n_envs), dtype=bool)
    lengths = []
    since = np.zeros(n_envs, dtype=int)
    for t in range(n_steps):
        envs.step(policy(envs.legal()))
        done[t] = envs.done.astype(bool)
        rewards[t] = terminal_rewards(envs.done, envs.won)
        since += 1
        lengths.extend(since[done[t]].tolist())
        since[done[t]] = 0
    envs.close()
    return rewards, done, lengths


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
