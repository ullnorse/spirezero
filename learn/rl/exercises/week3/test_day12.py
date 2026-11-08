import pytest
import torch

from conftest import exercise

day = exercise("week3.day12_baseline")
from common.minifight import MiniFight, State, win_rate  # noqa: E402


def test_features13_flags_lethal_and_kill():
    f = day.features13(day.PROBE)
    assert f.shape == (13,) and f[11] == 1.0 and f[12] == 0.0
    assert day.features13(day.PROBE_WIN)[11:13].tolist() == [1.0, 0.0]
    assert day.features13(State(enemy_hp=6, intent="bellow"))[11:13].tolist() == [0.0, 1.0]
    assert day.features13(State())[11:13].tolist() == [0.0, 0.0]
    assert day.features13(State(block=12))[11] == 0.0


def test_collect_shapes_and_returns():
    torch.manual_seed(0)
    X, A, L, G, wins = day.collect(MiniFight(0), day.ActorCritic(), 5)
    n = len(X)
    assert X.shape == (n, 13) and A.shape == L.shape[:1] == G.shape == (n,) and L.shape[1] == 3
    assert L[torch.arange(n), A].all() and 0 <= wins <= 5
    assert all(any(abs(abs(g) - 0.99 ** k) < 1e-5 for k in range(200)) for g in G.tolist())


def test_actor_critic_loss_terms():
    logits = torch.zeros(2, 3)
    legal = torch.tensor([[True, True, True], [False, True, True]])
    actions = torch.tensor([0, 1])
    G = torch.tensor([1.0, -1.0])
    values = torch.tensor([0.5, -0.5], requires_grad=True)
    loss, pg, vl, ent = day.actor_critic_loss(logits, values, legal, actions, G, ent_coef=0.0, vf_coef=1.0, use_critic=False)
    # log 1/3 and log 1/2; A = G.
    assert pg.item() == pytest.approx(-(torch.log(torch.tensor(1 / 3)) * 1 + torch.log(torch.tensor(0.5)) * -1).item() / 2)
    assert vl.item() == pytest.approx(0.25) and loss.item() == pytest.approx(pg.item())
    assert ent.item() == pytest.approx((torch.log(torch.tensor(3.0)) + torch.log(torch.tensor(2.0))).item() / 2)
    loss2, pg2, _, _ = day.actor_critic_loss(logits, values, legal, actions, G, ent_coef=0.0, vf_coef=1.0, use_critic=True)
    assert pg2.item() == pytest.approx(-(torch.log(torch.tensor(1 / 3)) * 0.5 + torch.log(torch.tensor(0.5)) * -0.5).item() / 2)
    assert loss2.item() == pytest.approx(pg2.item() + 0.25)
    loss2.backward()
    assert values.grad is not None  # the value term trains the critic, the policy term does not


def test_the_feature_unlocks_the_block_and_the_critic_adds_to_it():
    net, curve = day.train(episodes=3000, seed=0)
    assert len(curve) == 12 and curve[-1] > 0.6
    assert day.p_defend(net) > 0.5 and day.p_defend(net, day.PROBE_WIN) > 0.5
    assert win_rate(day.greedy_policy(net), 500, seed=3) > 0.78  # day 9's strike-first policy: 0.76
    old, _ = day.train(episodes=2000, feats=day.features, seed=0)
    assert day.p_defend(old, feats=day.features) < 0.5
