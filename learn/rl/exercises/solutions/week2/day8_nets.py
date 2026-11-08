"""Day 8: from a table to a net. Featurise the real observation, collect (features, outcome)
pairs, fit a value net with torch, and compare it with the coarse table. Reference solution."""

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
from solutions.week1.day2_observe import cards_first  # noqa: E402
from solutions.week2.day7_td import real_key  # noqa: E402

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
    obs = np.asarray(obs, dtype=np.uint8)
    n = len(obs)
    i16 = obs[:, :12].copy().view("<i2")
    hp, max_hp, block, energy, max_energy = (i16[:, k].astype(np.float32) for k in range(5))
    max_hp = np.maximum(max_hp, 1)
    max_energy = np.maximum(max_energy, 1)
    turn = obs[:, 12:14].copy().view("<u2")[:, 0].astype(np.float32)
    hand, draw, discard = (obs[:, k].astype(np.float32) for k in (16, 17, 18))
    slots = obs[:, INTENTS_AT:INTENTS_AT + INTENTS * INTENT_BYTES].reshape(n, INTENTS, INTENT_BYTES)
    dmg = slots[:, :, 1:3].copy().view("<i2")[:, :, 0].astype(np.float32)
    hits = slots[:, :, 3].astype(np.float32)
    attack = slots[:, :, 0] == 1
    incoming = (dmg * hits * attack).sum(axis=1)
    enemies = (slots[:, :, 0] > 0).sum(axis=1).astype(np.float32)
    plays = legal[:, :POTION_BASE]
    target = np.arange(POTION_BASE) % TARGET_SLOTS
    attacks = plays[:, target >= 1].sum(axis=1).astype(np.float32)
    skills = plays[:, target == 0].sum(axis=1).astype(np.float32)
    potions = legal[:, POTION_BASE:END_TURN].sum(axis=1).astype(np.float32)
    end = legal[:, END_TURN].astype(np.float32)
    scalars = np.stack([hp / max_hp, block / max_hp, energy / max_energy, turn / 20, hand / 10, draw / 40,
                        discard / 40, incoming / max_hp, enemies / 8, attacks / 10, skills / 10, potions / 6, end,
                        np.ones(n, np.float32)], axis=1)
    cards = np.zeros((n, N_CARDS), dtype=np.float32)
    ids = obs[:, HAND_AT:HAND_AT + 20].copy().view("<u2")
    kinds_table = obs[:, CARDS_AT:CARDS_AT + 64 * CARD_BYTES].reshape(n, 64, CARD_BYTES)[:, :, 0:2].copy().view("<u2")[:, :, 0]
    for i in range(n):
        k = int(hand[i])
        if k:
            cards[i, kinds_table[i, ids[i, :k]]] = 1.0
    return np.concatenate([scalars, cards], axis=1)


def collect(policy, n_envs=64, n_steps=2000, seed=0):
    """Rolls `policy(legal) -> actions` out and pairs every step's features with the outcome
    (+1/-1) of the fight it belonged to. Steps of fights that never end are dropped. Returns
    (X: m × N_FEATURES float32, y: m float32, keys: list of m real_key tuples)."""
    envs = make_envs(n_envs, seed=seed)
    pending = [[] for _ in range(n_envs)]
    X, y, keys = [], [], []
    for _ in range(n_steps):
        legal = envs.legal()
        feats = featurise(envs.obs, legal)
        for i in range(n_envs):
            pending[i].append((feats[i], real_key(envs.obs[i], legal[i])))
        envs.step(policy(legal))
        for i in np.flatnonzero(envs.done):
            g = 1.0 if envs.won[i] else -1.0
            for f, k in pending[i]:
                X.append(f)
                y.append(g)
                keys.append(k)
            pending[i] = []
    envs.close()
    return np.stack(X), np.array(y, dtype=np.float32), keys


class ValueNet(nn.Module):
    """features → hidden (ReLU) → hidden (ReLU) → 1, squashed by tanh into (-1, 1)."""

    def __init__(self, n_features=N_FEATURES, hidden=128):
        """Builds the three Linear layers (`self.body` as an nn.Sequential)."""
        super().__init__()
        self.body = nn.Sequential(nn.Linear(n_features, hidden), nn.ReLU(), nn.Linear(hidden, hidden), nn.ReLU(),
                                  nn.Linear(hidden, 1))

    def forward(self, x):
        """x: batch × n_features → batch values in (-1, 1)."""
        return torch.tanh(self.body(x)).squeeze(-1)


def fit(net, X, y, epochs=10, batch=512, lr=1e-3, seed=0, device=DEVICE):
    """Minimises the mean squared error with Adam over shuffled minibatches. Returns the mean
    training loss per epoch."""
    torch.manual_seed(seed)
    net.to(device)
    X = torch.as_tensor(X, device=device)
    y = torch.as_tensor(y, device=device)
    opt = torch.optim.Adam(net.parameters(), lr=lr)
    losses = []
    for _ in range(epochs):
        perm = torch.randperm(len(X), device=device)
        total = 0.0
        for start in range(0, len(X), batch):
            idx = perm[start:start + batch]
            loss = ((net(X[idx]) - y[idx]) ** 2).mean()
            opt.zero_grad()
            loss.backward()
            opt.step()
            total += loss.item() * len(idx)
        losses.append(total / len(X))
    return losses


def mse(net, X, y, device=DEVICE):
    """Mean squared error of the net on (X, y), without gradients."""
    with torch.no_grad():
        pred = net(torch.as_tensor(X, device=device)).cpu().numpy()
    return float(((pred - y) ** 2).mean())


def table_mse(train_keys, train_y, test_keys, test_y):
    """The coarse table's held-out error: predict each test key by the mean outcome of its
    training visits (0 for a key never seen). Returns (mse, number of unseen test rows)."""
    total, count = {}, {}
    for k, g in zip(train_keys, train_y):
        total[k] = total.get(k, 0.0) + g
        count[k] = count.get(k, 0) + 1
    pred = np.array([total[k] / count[k] if k in count else 0.0 for k in test_keys])
    unseen = sum(k not in count for k in test_keys)
    return float(((pred - test_y) ** 2).mean()), unseen


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
