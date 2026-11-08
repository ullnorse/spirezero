import numpy as np
import pytest

from conftest import exercise

day = exercise("week1.day1_loop")


def test_legal_indices_reads_bits_little_endian_across_words():
    mask = np.array([0b101, 1 << 3, 1 << 11], dtype=np.uint64)
    assert day.legal_indices(mask).tolist() == [0, 2, 67, 139]


def test_legal_indices_agrees_with_the_env():
    envs = day.make_envs(8, seed=2)
    legal = envs.legal()
    for i in range(envs.n):
        assert day.legal_indices(envs.masks[i]).tolist() == np.flatnonzero(legal[i]).tolist()
    envs.close()


def test_random_actions_are_legal_and_vary():
    envs = day.make_envs(16, seed=3)
    rng = np.random.default_rng(0)
    lowest = 0
    for _ in range(100):
        legal = envs.legal()
        actions = day.random_actions(legal, rng)
        assert actions.shape == (16,)
        assert legal[np.arange(16), actions].all()
        lowest += int((actions == legal.argmax(axis=1)).sum())
        envs.step(actions)
    assert lowest < 1600 * 0.6, "always picking the lowest legal index is not random"
    envs.close()


def test_run_random_counts_episodes_and_wins():
    r = day.run_random(n_envs=64, n_steps=3000, seed=0)
    assert r["episodes"] > 2000
    assert r["wins"] == pytest.approx(r["win_rate"] * r["episodes"])
    assert 0.28 <= r["win_rate"] <= 0.40, "a random legal agent wins about a third of Act 1 fights"
    assert r["steps_per_s"] > 0
