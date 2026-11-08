import math

import numpy as np
import pytest

from conftest import exercise

day = exercise("week3.day13_eval")


def test_standard_error_and_separation():
    assert day.standard_error(0.5, 100) == pytest.approx(0.05)
    assert day.standard_error(0.0, 10) == 0.0
    assert day.fights_to_separate(0.60, 0.65) == 719
    assert day.fights_to_separate(0.34, 0.58) < 40
    assert day.fights_to_separate(0.5, 0.5) == math.inf


def test_evaluate_counts_exactly_n_fights_and_is_reproducible():
    a = day.evaluate(day.cards_first_policy, n_fights=200, n_envs=32, seed=3)
    b = day.evaluate(day.cards_first_policy, n_fights=200, n_envs=32, seed=3)
    assert a["n"] == b["n"] == 200
    assert a["win_rate"] == b["win_rate"] and a["by_encounter"] == b["by_encounter"]
    assert sum(n for _, n in a["by_encounter"].values()) == 200
    assert sum(w for w, _ in a["by_encounter"].values()) == round(a["win_rate"] * 200)
    assert a["se"] == pytest.approx(day.standard_error(a["win_rate"], 200))
    assert 5 < a["mean_length"] < 200
    c = day.evaluate(day.cards_first_policy, n_fights=200, n_envs=32, seed=4)
    assert c["by_encounter"] != a["by_encounter"]


def test_baselines_rank_as_expected():
    r = day.evaluate(day.random_legal, n_fights=300, n_envs=64)
    c = day.evaluate(day.cards_first_policy, n_fights=300, n_envs=64)
    assert 0.2 < r["win_rate"] < 0.5 < c["win_rate"]
    diff, se, rows = compare = day.compare(r, c)
    assert diff == pytest.approx(c["win_rate"] - r["win_rate"]) and diff > 3 * se
    assert rows and all(len(row) == 5 for row in rows)
    assert all(rows[i][2] - rows[i][1] >= rows[i + 1][2] - rows[i + 1][1] for i in range(len(rows) - 1))


def test_greedy_wraps_a_net():
    ppo = exercise("week2.day10_ppo")
    net = ppo.ActorCritic().to(ppo.DEVICE)
    policy = day.greedy(net)
    envs = day.make_envs(4, seed=1)
    legal = envs.legal()
    actions = policy(envs.obs, legal)
    assert actions.shape == (4,) and legal[np.arange(4), actions].all()
    assert (policy(envs.obs, legal) == actions).all()
    envs.close()
