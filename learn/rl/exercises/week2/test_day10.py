import numpy as np
import pytest
import torch

from conftest import exercise

day = exercise("week2.day10_ppo")


def test_gae_cuts_at_done_and_bootstraps_at_the_end():
    rew = torch.tensor([[0.0], [1.0], [0.0]])
    val = torch.tensor([[0.5], [0.5], [0.5]])
    done = torch.tensor([[0.0], [1.0], [0.0]])
    adv, ret = day.gae(rew, val, done, torch.tensor([0.5]), gamma=1.0, lam=1.0)
    # t=1 ends: delta = 1 - 0.5; t=0: delta = 0 + 0.5 - 0.5 = 0 plus carried 0.5; t=2 bootstraps on last_value.
    assert adv[:, 0].tolist() == pytest.approx([0.5, 0.5, 0.0])
    assert ret[:, 0].tolist() == pytest.approx([1.0, 1.0, 0.5])
    adv2, _ = day.gae(rew, val, done, torch.tensor([0.5]), gamma=0.9, lam=0.5)
    assert adv2[1, 0] == pytest.approx(0.5)
    assert adv2[0, 0] == pytest.approx((0.9 * 0.5 - 0.5) + 0.9 * 0.5 * 0.5)
    assert adv2[2, 0] == pytest.approx(0.9 * 0.5 - 0.5)


def test_ppo_loss_clips_the_ratio():
    old = torch.zeros(4)
    adv = torch.tensor([1.0, 1.0, -1.0, -1.0])
    loss, clipfrac, kl = day.ppo_loss(old.clone(), old, adv, clip=0.2)
    assert loss.item() == pytest.approx(0.0) and clipfrac == 0.0 and kl == pytest.approx(0.0)
    # Pushing a good action far up is clipped at 1.2; pushing it down is not clipped (pessimistic min).
    new = torch.log(torch.tensor([2.0, 0.5, 2.0, 0.5]))
    loss, clipfrac, kl = day.ppo_loss(new, old, adv, clip=0.2)
    assert loss.item() == pytest.approx(-(1.2 + 0.5 - 2.0 - 0.8) / 4)
    assert clipfrac == 1.0


def test_actor_critic_masks_logits():
    net = day.ActorCritic()
    x = torch.randn(3, day.N_FEATURES)
    legal = torch.zeros(3, 140, dtype=torch.bool)
    legal[:, [5, 128]] = True
    dist, v = net(x, legal)
    assert v.shape == (3,)
    assert dist.probs[:, [5, 128]].sum(dim=1).tolist() == pytest.approx([1.0, 1.0, 1.0])
    assert (dist.probs[:, :5] == 0).all()


def test_rollout_only_takes_legal_actions_and_shapes_match():
    envs = day.make_envs(8, seed=3)
    net = day.ActorCritic().to(day.DEVICE)
    b = day.rollout(envs, net, 20)
    envs.close()
    assert b["x"].shape == (20, 8, day.N_FEATURES) and b["legal"].shape == (20, 8, 140)
    assert b["act"].shape == b["logp"].shape == b["val"].shape == b["rew"].shape == b["done"].shape == (20, 8)
    assert b["last_val"].shape == (8,)
    assert b["legal"].gather(2, b["act"].unsqueeze(-1).long()).all()
    assert ((b["rew"] != 0) == (b["done"] == 1)).all()
    assert b["episodes"] == int(b["done"].sum()) and 0 <= b["wins"] <= b["episodes"]


def test_train_improves_on_random_quickly():
    net, history = day.train(iterations=15, n_envs=64, T=128, seed=0)
    assert len(history) == 15
    for h in history:
        assert np.isfinite(h["loss"]) and 0 <= h["clipfrac"] <= 1 and abs(h["approx_kl"]) < 0.1
    assert history[-1]["entropy"] < history[0]["entropy"]
    late = np.mean([h["win_rate"] for h in history[-3:]])
    assert late > 0.5, "random wins 0.34; a few iterations of PPO should pass 0.5"
    wr, n = day.evaluate(net, n_steps=500, seed=7)
    assert n > 800 and wr > 0.5
