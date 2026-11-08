import numpy as np
import pytest

from conftest import exercise

day = exercise("week3.day15_shaping")


def obs_with_hp(hp, max_hp=80):
    o = np.zeros((len(hp), 972), dtype=np.uint8)
    o[:, 0:2] = np.array(hp, dtype="<i2").view(np.uint8).reshape(-1, 2)
    o[:, 2:4] = np.array([max_hp] * len(hp), dtype="<i2").view(np.uint8).reshape(-1, 2)
    return o


def test_hp_fraction_reads_the_header():
    assert day.hp_fraction(obs_with_hp([80, 40, 0])).tolist() == pytest.approx([1.0, 0.5, 0.0])


def test_shaped_rewards():
    before = obs_with_hp([80, 80, 80, 40])
    after = obs_with_hp([72, 80, 80, 40])
    done = np.array([0, 1, 1, 0], dtype=np.uint8)
    won = np.array([0, 1, 0, 0], dtype=np.uint8)
    assert day.shaped("sparse", before, after, done, won).tolist() == [0.0, 1.0, -1.0, 0.0]
    assert day.shaped("hp_loss", before, after, done, won, scale=0.1).tolist() == pytest.approx([-0.01, 1.0, -1.0, 0.0])
    assert day.shaped("win_hp_bonus", before, after, done, won, scale=0.1).tolist() == pytest.approx([0.0, 1.1, -1.0, 0.0])
    pot = day.shaped("potential", before, after, done, won, scale=0.1, gamma=1.0)
    assert pot.tolist() == pytest.approx([-0.01, 0.9, -1.1, 0.0])
    assert day.shaped("potential", before, after, done, won, scale=0.1, gamma=0.5)[0] == pytest.approx(0.5 * 0.09 - 0.1)
    with pytest.raises(ValueError):
        day.shaped("damage", before, after, done, won)


def test_potential_term_telescopes_to_minus_phi0():
    assert day.telescoped([0.1, 0.08, 0.05], gamma=1.0) == pytest.approx(-0.1)
    assert day.telescoped([0.1, 0.08, 0.05], gamma=0.99) == pytest.approx(-0.1)
    assert day.telescoped([0.03], gamma=0.9) == pytest.approx(-0.03)


def test_rollout_uses_the_shaped_reward_and_counts_hp():
    envs = day.make_envs(8, seed=3)
    net = day.ActorCritic().to(day.DEVICE)
    b = day.rollout(envs, net, 40, "hp_loss", scale=0.1)
    envs.close()
    assert b["rew"].shape == (40, 8) and 0 <= b["hp_won"] <= b["wins"] <= b["episodes"]
    rew, done = b["rew"].cpu().numpy(), b["done"].cpu().numpy()
    assert (rew[done == 0] <= 0).all() and (rew[done == 0] > -0.2).all()
    assert (np.abs(rew[done == 1]) == 1).all()
    assert (rew[done == 0] < 0).any(), "some step lost HP"


def test_train_and_evaluate_run_for_every_kind():
    for kind in day.KINDS:
        net, history = day.train(kind, iterations=2, n_envs=32, T=64)
        assert len(history) == 2 and np.isfinite(history[-1]["loss"])
    wr, hp, length = day.evaluate(net, n_fights=100, n_envs=32)
    assert 0 <= wr <= 1 and 0 <= hp <= 1 and length > 1
