import numpy as np
import pytest
import torch

from conftest import exercise

day = exercise("week2.day8_nets")
cards_first = exercise("week1.day2_observe").cards_first


@pytest.fixture(scope="module")
def data():
    X, y, keys = day.collect(cards_first, n_envs=64, n_steps=600, seed=0)
    Xt, yt, kt = day.collect(cards_first, n_envs=64, n_steps=200, seed=1)
    return X, y, keys, Xt, yt, kt


def test_featurise_shapes_and_ranges():
    envs = day.make_envs(16, seed=2)
    legal = envs.legal()
    F = day.featurise(envs.obs, legal)
    assert F.shape == (16, day.N_FEATURES) and F.dtype == np.float32
    hp = envs.obs[:, 0:2].copy().view("<i2")[:, 0]
    max_hp = envs.obs[:, 2:4].copy().view("<i2")[:, 0]
    assert F[:, 0] == pytest.approx(hp / max_hp)
    assert (F[:, 2] == 1.0).all(), "full energy on turn 1"
    assert (F[:, day.N_SCALARS - 1] == 1.0).all(), "the constant"
    assert (F[:, 12] == legal[:, 128]).all()
    hand = envs.obs[:, 16]
    kinds_in_hand = F[:, day.N_SCALARS:].sum(axis=1)
    assert (kinds_in_hand >= 1).all() and (kinds_in_hand <= hand).all(), "one hot per distinct kind in hand"
    envs.close()


def test_collect_pairs_every_step_with_its_fights_outcome(data):
    X, y, keys, *_ = data
    assert X.shape[1] == day.N_FEATURES and len(X) == len(y) == len(keys)
    assert set(np.unique(y)) <= {-1.0, 1.0}
    assert 0.15 < y.mean() < 0.5  # cards first wins about 60% of fights
    assert len(X) > 5000


def test_value_net_outputs_in_range():
    net = day.ValueNet()
    v = net(torch.randn(5, day.N_FEATURES))
    assert v.shape == (5,) and (v.abs() < 1).all()


def test_fit_lowers_the_loss_and_beats_the_table(data):
    X, y, keys, Xt, yt, kt = data
    net = day.ValueNet()
    losses = day.fit(net, X, y, epochs=8, seed=0)
    assert len(losses) == 8 and losses[-1] < losses[0]
    base = float(((yt - y.mean()) ** 2).mean())
    table, unseen = day.table_mse(keys, y, kt, yt)
    net_mse = day.mse(net, Xt, yt)
    assert table < base
    assert net_mse < table - 0.05, (base, table, net_mse)


def test_table_mse_predicts_unseen_keys_as_zero():
    m, unseen = day.table_mse(["a", "a", "b"], np.array([1.0, -1.0, 1.0]), ["a", "b", "c"], np.array([1.0, 1.0, 1.0]))
    assert unseen == 1
    assert m == pytest.approx(((0 - 1) ** 2 + 0 + (0 - 1) ** 2) / 3)
