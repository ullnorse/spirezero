import numpy as np
import pytest

from conftest import exercise

day = exercise("week1.day3_returns")


def test_returns_accumulate_backwards():
    assert day.returns([0, 0, 0, 1], 0.9) == pytest.approx([0.729, 0.81, 0.9, 1.0])
    assert day.returns([1, 2, 3], 1.0) == pytest.approx([6, 5, 3])
    assert day.returns([], 0.5) == []


def test_terminal_rewards_are_plus_minus_one_at_the_end_only():
    r = day.terminal_rewards(np.array([1, 1, 0], dtype=np.uint8), np.array([1, 0, 0], dtype=np.uint8))
    assert r.tolist() == [1.0, -1.0, 0.0]


def test_episode_returns_split_each_env_at_done():
    # env 0: ends at t=1 (loss) and t=3 (win); env 1: ends at t=2 (win), tail unfinished.
    rewards = np.array([[0, 0], [-1, 0], [0, 1], [1, 0]], dtype=float)
    done = np.array([[0, 0], [1, 0], [0, 1], [1, 0]], dtype=bool)
    assert day.episode_returns(rewards, done, 1.0) == pytest.approx([-1.0, 1.0, 1.0])
    # Discounting: episode lengths 2, 3 and 2, so G_0 = gamma^(len-1) * terminal reward.
    assert day.episode_returns(rewards, done, 0.5) == pytest.approx([-0.5, 0.25, 0.5])


def test_hp_shaped_charges_for_hp_lost_except_on_the_last_step():
    before = np.zeros((2, 972), dtype=np.uint8)
    after = np.zeros((2, 972), dtype=np.uint8)
    before[:, 0:2] = np.array([50, 0], dtype=np.uint8)
    after[0, 0:2] = np.array([44, 0], dtype=np.uint8)
    after[1, 0:2] = np.array([80, 0], dtype=np.uint8)  # a new fight's full HP
    r = day.hp_shaped(before, after, np.array([0, 1], dtype=np.uint8), np.array([0, 0], dtype=np.uint8), scale=0.01)
    assert r.tolist() == pytest.approx([-0.06, -1.0])


def test_collect_logs_a_consistent_rollout():
    from conftest import exercise as _ex
    cards_first = _ex("week1.day2_observe").cards_first
    rewards, done, lengths = day.collect(cards_first, n_envs=64, n_steps=1000, seed=0)
    assert rewards.shape == (1000, 64) and done.shape == (1000, 64)
    assert ((rewards != 0) == done).all()
    assert len(lengths) == int(done.sum()) > 1000
    g = day.episode_returns(rewards, done, 1.0)
    assert len(g) == len(lengths)
    assert 0.15 <= np.mean(g) <= 0.5
    # Shorter wins are worth more once gamma < 1, so the discounted mean is smaller.
    assert np.mean(day.episode_returns(rewards, done, 0.99)) < np.mean(g)
