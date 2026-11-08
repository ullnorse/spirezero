"""Day 15: reward shaping on the real env. Four reward functions over the same rollout data,
the telescoping check for the potential-based one, and PPO trained with each on the same seeds
and judged by the greedy win rate and the HP left after wins.

Run:  .venv/bin/python learn/rl/exercises/week3/day15_shaping.py [iterations]
"""

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve()
while not (ROOT / "Cargo.toml").exists():
    ROOT = ROOT.parent
sys.path[:0] = [str(ROOT / "python"), str(ROOT / "learn/rl/exercises")]

import numpy as np  # noqa: E402
import torch  # noqa: E402

from common import make_envs  # noqa: E402
from week2.day8_nets import DEVICE, featurise  # noqa: E402
from week2.day10_ppo import ActorCritic, update  # noqa: E402

KINDS = ("sparse", "hp_loss", "win_hp_bonus", "potential")


def hp_fraction(obs):
    """HP / max HP per env from the header (i16 at bytes 0 and 2), as float64; 0 for a max HP
    of 0."""
    raise NotImplementedError  # TODO


def shaped(kind, obs_before, obs_after, done, won, scale=0.1, gamma=0.99):
    """One step's reward per env, by `kind`:
    sparse: +1 won, -1 other end, 0 otherwise;
    hp_loss: sparse - scale · max(0, hp fraction lost this step); 0 lost on an ending step
    (the next observation is a new fight's);
    win_hp_bonus: sparse + scale · hp fraction of obs_before on a step that won (the HP the
    fight ended with is the HP before the killing blow);
    potential: sparse + gamma · Φ(s') - Φ(s) with Φ = scale · hp fraction, and Φ(s') = 0 on an
    ending step (the terminal state's potential is zero).
    `done` and `won` are uint8 arrays; returns float32."""
    raise NotImplementedError  # TODO


def telescoped(phis, gamma=0.99):
    """What a potential-based term adds to the discounted return of one episode: for the
    sequence Φ(s_0) ... Φ(s_{T-1}) (the last state before the terminal one, whose Φ is 0),
    Σ_t γ^t (γ Φ(s_{t+1}) - Φ(s_t)), which telescopes to -Φ(s_0). Returns the sum, computed
    term by term so the test can check the claim."""
    raise NotImplementedError  # TODO


def rollout(envs, net, T, kind, device=DEVICE, scale=0.1, gamma=0.99):
    """Day 10's rollout with `shaped(kind, ...)` as the reward: the observation before and
    after each step are both needed, so copy `envs.obs` before stepping. Also counts
    `hp_won`: the sum of hp fractions (before the ending step) over the fights won."""
    raise NotImplementedError  # TODO


def train(kind, iterations=20, n_envs=64, T=128, lr=3e-4, seed=0, scale=0.1, log=None, **kw):
    """PPO with reward `kind`, otherwise day 10's `train`. Returns (net, history)."""
    raise NotImplementedError  # TODO


def evaluate(net, n_fights=512, n_envs=64, seed=7):
    """Greedy play over the first `n_fights` fights of `make_envs(n_envs, seed)`. Returns
    (win_rate, mean hp fraction left after the won fights, mean fight length): the numbers the
    reward functions are judged by, which none of them is."""
    raise NotImplementedError  # TODO


if __name__ == "__main__":
    iterations = int(sys.argv[1]) if len(sys.argv) > 1 else 40
    print(f"potential term over an episode with Φ = 0.1 · (1.0, 0.8, 0.5): {telescoped([0.1, 0.08, 0.05]):+.4f} (-Φ(s_0) = -0.1)")
    for kind in KINDS:
        net, history = train(kind, iterations)
        wr, hp, length = evaluate(net)
        print(f"{kind:13s} greedy win {wr:.3f}  HP left after wins {hp:.2f}  length {length:.1f}  "
              f"(training win {history[-1]['win_rate']:.3f}, entropy {history[-1]['entropy']:.2f})")
