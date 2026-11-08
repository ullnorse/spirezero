import numpy as np
import pytest
import torch

from conftest import exercise

day = exercise("week4.day17_runppo")
d16 = exercise("week4.day16_runenv")


def test_featurise_run_shapes_and_scalars():
    envs = d16.make_run_envs(4)
    f = day.featurise_run(envs.obs)
    h = d16.run_header(envs.obs[0])
    assert f.shape == (4, day.N_FEATURES) and f.dtype == np.float32
    assert f[0, 0] == pytest.approx(h["hp"] / h["max_hp"]) and f[0, 1] == pytest.approx(h["gold"] / 500)
    assert f[0, 4] == pytest.approx(h["deck_size"] / 40) and f[0, 9] == 1.0
    phase = f[:, 10:10 + len(day.PHASES)]
    assert (phase.sum(axis=1) == 1).all() and day.PHASES[int(phase[0].argmax())] == h["phase"]
    deck = f[:, day.N_SCALARS:day.N_SCALARS + day.N_CARDS]
    assert deck[0].sum() <= h["deck_size"] and deck[0].sum() >= 3 and deck[0, 0] == 0
    relics = f[:, day.N_SCALARS + 2 * day.N_CARDS:]
    assert relics.shape[1] == day.N_RELICS and relics[0].sum() == h["relics"]
    envs.close()


def test_run_reward_kinds():
    done = np.array([0, 1, 1, 0], dtype=np.uint8)
    won = np.array([0, 1, 0, 0], dtype=np.uint8)
    before, after = np.array([3, 9, 4, 5]), np.array([4, 0, 0, 5])
    assert day.run_reward("sparse", before, after, done, won).tolist() == [0.0, 1.0, -1.0, 0.0]
    assert day.run_reward("floor", before, after, done, won, scale=0.1).tolist() == pytest.approx([0.1, 1.0, 0.0, 0.0])
    with pytest.raises(ValueError):
        day.run_reward("gold", before, after, done, won)


def test_rollout_over_runs_only_takes_legal_run_level_actions():
    envs = d16.make_run_envs(8, threads=2)
    net = day.RunActorCritic().to(day.DEVICE)
    b = day.rollout(envs, net, 30)
    envs.close()
    assert b["x"].shape == (30, 8, day.N_FEATURES) and b["legal"].shape == (30, 8, day.RUN_ACTIONS)
    assert b["legal"].gather(2, b["act"].unsqueeze(-1).long()).all() and (b["act"] >= d16.RUN_BASE).all()
    assert b["episodes"] == len(b["floors"]) == int(b["done"].sum())
    assert (b["rew"] >= 0).all() and (b["rew"] > 0).any()


def test_train_climbs_past_random():
    net, history = day.train(iterations=20, seed=0)
    assert len(history) == 20 and all(np.isfinite(h["loss"]) for h in history)
    assert np.mean([h["mean_floor"] for h in history[-5:]]) > np.mean([h["mean_floor"] for h in history[:5]])
    mean, se, floors, ends = d16.measure_floors(day.greedy(net), n_runs=150, seeds=(5000, 6000))
    rand, _, _, _ = d16.measure_floors(d16.random_run_policy, n_runs=150, seeds=(5000, 6000))
    assert len(floors) == 150 and mean > rand + 0.5
