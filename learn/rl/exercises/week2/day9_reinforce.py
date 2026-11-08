"""Day 9: REINFORCE with a baseline on the mini fight, with a torch policy net and a masked
categorical distribution.

Run:  .venv/bin/python learn/rl/exercises/week2/day9_reinforce.py
Test: .venv/bin/python -m pytest learn/rl/exercises/week2/test_day9.py
"""

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve()
while not (ROOT / "Cargo.toml").exists():
    ROOT = ROOT.parent
sys.path[:0] = [str(ROOT / "python"), str(ROOT / "learn/rl/exercises")]

import numpy as np  # noqa: E402
import torch  # noqa: E402
from torch import nn  # noqa: E402

from common.minifight import ENEMY_HP, PLAYER_HP, STRIKE_DMG, MiniFight, State, win_rate  # noqa: E402

N_FEATURES = 11
MOVES = ("chomp", "thrash", "bellow")


def features(s):
    """11 floats for a mini-fight state: hp/28, block/28, energy/3, strikes/5, defends/5,
    enemy (hp + block)/48, strength/12, damage that would get through the block / 28, and a
    one-hot of the intent (chomp, thrash, bellow)."""
    raise NotImplementedError  # TODO


class PolicyNet(nn.Module):
    """features → 64 (tanh) → 3 logits."""

    def __init__(self, hidden=64):
        super().__init__()
        self.body = nn.Sequential(nn.Linear(N_FEATURES, hidden), nn.Tanh(), nn.Linear(hidden, 3))

    def forward(self, x):
        return self.body(x)


def masked_dist(logits, legal):
    """A Categorical over the legal actions: illegal logits are set to -1e9 before the
    softmax. `legal` is a bool tensor of the same shape."""
    raise NotImplementedError  # TODO


def run_episode(fight, net):
    """Plays one fight by sampling from the net. Returns (log_probs: list of 0-d tensors,
    rewards: list of floats, final state)."""
    raise NotImplementedError  # TODO


def returns(rewards, gamma):
    out, g = [0.0] * len(rewards), 0.0
    for t in range(len(rewards) - 1, -1, -1):
        g = rewards[t] + gamma * g
        out[t] = g
    return out


def reinforce_loss(logps, gs, baseline):
    """-(sum over steps of log π(a_t | s_t) · (G_t - baseline)) for one episode."""
    raise NotImplementedError  # TODO


def train(episodes=3000, batch=16, lr=0.01, gamma=0.99, baseline=True, seed=0, window=250):
    """REINFORCE: every `batch` episodes, one Adam step on the mean episode loss. The baseline
    is a running mean of episode returns (0.9 decay), used only when `baseline` is True.
    Returns (net, training win rate per `window` episodes)."""
    raise NotImplementedError  # TODO


def greedy_policy(net):
    """`policy(state, legal, rng)` that plays the net's most likely legal action."""
    def policy(s, legal, rng):
        with torch.no_grad():
            logits = net(torch.as_tensor(features(s))).masked_fill(~torch.tensor(legal), -1e9)
        return int(logits.argmax())
    return policy


# HP 7 with 5 block, 1 energy, a Strike and a Defend in hand, Chomp (12) incoming, Worm at 30: only Defend survives.
PROBE = State(player_hp=7, block=5, energy=1, enemy_hp=30, intent="chomp", strikes=1, defends=1, draw_s=2, draw_d=3,
              disc_s=2, disc_d=1, turn=2)


def p_defend(net, s=PROBE):
    """P(Defend | s) under the net."""
    raise NotImplementedError  # TODO


if __name__ == "__main__":
    for baseline in (False, True):
        net, curve = train(baseline=baseline)
        print(f"baseline {baseline!s:5}: training win rate {curve[0]:.2f} → {curve[-1]:.2f}  "
              f"greedy {win_rate(greedy_policy(net), 1000, seed=3):.3f}  P(Defend | lethal Chomp) {p_defend(net):.2f}")
