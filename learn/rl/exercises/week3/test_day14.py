import numpy as np
import torch

from conftest import exercise

day = exercise("week3.day14_features")


def test_hand_slots_reads_kinds_upgrades_and_costs():
    envs = day.make_envs(8, seed=2)
    slots = day.hand_slots(envs.obs)
    size = envs.obs[:, 16]
    assert slots.shape == (8, 10, 3) and slots.dtype == np.int64
    for i in range(8):
        assert (slots[i, :size[i], 0] > 0).all() and (slots[i, size[i]:] == 0).all()
        assert (slots[i, :size[i], 2] >= -1).all() and (slots[i, :size[i], 2] <= 5).all()
    # the hand's kinds are the kinds of the hand's card ids in the cards table
    table = envs.obs[:, day.CARDS_AT:day.CARDS_AT + 64 * day.CARD_BYTES].reshape(8, 64, day.CARD_BYTES)
    ids = envs.obs[:, day.HAND_AT:day.HAND_AT + 20].copy().view("<u2")
    kinds = table[:, :, 0:2].copy().view("<u2")[:, :, 0]
    assert slots[0, 0, 0] == kinds[0, ids[0, 0]]
    f = day.featurise14(envs.obs, envs.legal())
    assert f.shape == (8, day.N_FEATURES) and f.dtype == np.float32
    assert (f[:, day.N_SCALARS::3] == slots[:, :, 0]).all()
    envs.close()


def test_slot_net_is_equivariant_under_hand_permutations():
    torch.manual_seed(0)
    net = day.SlotNet().to(day.DEVICE)
    envs = day.make_envs(16, seed=5)
    legal = envs.legal()
    x = torch.as_tensor(day.featurise14(envs.obs, legal), device=day.DEVICE)
    legal_t = torch.as_tensor(legal, device=day.DEVICE)
    envs.close()
    dist, v = net(x, legal_t)
    assert v.shape == (16,) and dist.probs.shape == (16, 140)
    assert (dist.probs[~legal_t] == 0).all()
    play, other = day.permutation_test(net, x, legal_t, 0, 1)
    assert play < 1e-4 and other < 1e-4
    play, other = day.permutation_test(net, x, legal_t, 2, 4)
    assert play < 1e-4 and other < 1e-4
    # A flat head is not: day 10's net over the same features (padded) moves everything.
    flat = day.ActorCritic(n_features=day.N_FEATURES).to(day.DEVICE)
    play, other = day.permutation_test(flat, x, legal_t, 0, 1)
    assert play > 1e-3 or other > 1e-3


def test_train_and_evaluate_with_the_slot_net():
    net, history = day.train(day.SlotNet(), day.featurise14, iterations=15, n_envs=64, T=128, seed=0)
    assert len(history) == 15 and np.mean([h["win_rate"] for h in history[-3:]]) > 0.5
    wr, n = day.evaluate(net, day.featurise14, n_fights=256)
    assert n == 256 and wr > 0.5
