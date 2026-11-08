"""Day 8: from a table to a net. Featurise the real observation, collect (features, outcome)
pairs, fit a value net with torch, and compare it with the coarse table.

Run:  .venv/bin/python learn/rl/exercises/week2/day8_nets.py
Test: .venv/bin/python -m pytest learn/rl/exercises/week2/test_day8.py
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

from common import (CARD_BYTES, CARDS_AT, END_TURN, HAND_AT, INTENT_BYTES, INTENTS, INTENTS_AT,  # noqa: E402
                    POTION_BASE, TARGET_SLOTS, make_envs, vocab)
from week1.day2_observe import cards_first  # noqa: E402
from week2.day7_td import real_key  # noqa: E402

N_CARDS = len(vocab("cards"))
N_SCALARS = 14
N_FEATURES = N_SCALARS + N_CARDS
DEVICE = "cuda" if torch.cuda.is_available() else "cpu"


def featurise(obs, legal):
    """Float features for a batch: obs is n × 972 uint8, legal n × 140 bool. Returns an
    n × N_FEATURES float32 array. The first N_SCALARS columns, in order:
    hp/max_hp, block/max_hp, energy/max_energy, turn/20, hand/10, draw/40, discard/40,
    incoming/max_hp (sum of damage × hits over attack intents), enemies with an intent / 8,
    legal attacks/10, legal skills/10, legal potions/6, End turn legal, a constant 1.
    Then a multi-hot over card kinds: a 1 for every kind in hand (ids at HAND_AT, kinds in
    the cards table)."""
    raise NotImplementedError  # TODO


def collect(policy, n_envs=64, n_steps=2000, seed=0):
    """Rolls `policy(legal) -> actions` out and pairs every step's features with the outcome
    (+1/-1) of the fight it belonged to. Steps of fights that never end are dropped. Returns
    (X: m × N_FEATURES float32, y: m float32, keys: list of m real_key tuples)."""
    raise NotImplementedError  # TODO


class ValueNet(nn.Module):
    """features → hidden (ReLU) → hidden (ReLU) → 1, squashed by tanh into (-1, 1)."""

    def __init__(self, n_features=N_FEATURES, hidden=128):
        """Builds the three Linear layers (`self.body` as an nn.Sequential)."""
        raise NotImplementedError  # TODO

    def forward(self, x):
        """x: batch × n_features → batch values in (-1, 1)."""
        raise NotImplementedError  # TODO


def fit(net, X, y, epochs=10, batch=512, lr=1e-3, seed=0, device=DEVICE):
    """Minimises the mean squared error with Adam over shuffled minibatches. Returns the mean
    training loss per epoch."""
    raise NotImplementedError  # TODO


def mse(net, X, y, device=DEVICE):
    """Mean squared error of the net on (X, y), without gradients."""
    with torch.no_grad():
        pred = net(torch.as_tensor(X, device=device)).cpu().numpy()
    return float(((pred - y) ** 2).mean())


def table_mse(train_keys, train_y, test_keys, test_y):
    """The coarse table's held-out error: predict each test key by the mean outcome of its
    training visits (0 for a key never seen). Returns (mse, number of unseen test rows)."""
    raise NotImplementedError  # TODO


if __name__ == "__main__":
    X, y, keys = collect(cards_first, n_steps=2000, seed=0)
    Xt, yt, kt = collect(cards_first, n_steps=500, seed=1)
    print(f"{len(X)} training rows, {len(Xt)} test rows, {N_FEATURES} features, device {DEVICE}")
    net = ValueNet()
    losses = fit(net, X, y, epochs=10)
    base = float(((yt - y.mean()) ** 2).mean())
    t_mse, unseen = table_mse(keys, y, kt, yt)
    print(f"loss per epoch {[round(l, 3) for l in losses]}")
    print(f"held-out MSE: predict the mean {base:.3f}  coarse table {t_mse:.3f} ({unseen} unseen rows)  net {mse(net, Xt, yt):.3f}")
