import numpy as np
import pytest

from conftest import exercise

day = exercise("week2.day7_td")
from common.minifight import heuristic_policy  # noqa: E402

coarse_key = exercise("week2.day6_qlearn").coarse_key


def test_mc_values_are_mean_returns_per_key():
    V, counts = day.mc_values(heuristic_policy, coarse_key, 300, seed=0)
    assert set(V) == set(counts) and all(-1 <= v <= 1 for v in V.values())
    assert sum(counts.values()) > 300 * 10
    assert abs(day.start_value(V, heuristic_policy, coarse_key, 500) - 0.6) < 0.15


def test_td0_is_biased_toward_zero_early_and_converges_late():
    early = day.td0_values(heuristic_policy, coarse_key, 300, alpha=0.1, seed=0)
    assert abs(day.start_value(early, heuristic_policy, coarse_key, 500)) < 0.2
    late = day.td0_values(heuristic_policy, coarse_key, 20000, alpha=0.1, seed=0)
    assert abs(day.start_value(late, heuristic_policy, coarse_key, 500) - 0.6) < 0.12


def test_td0_update_rule():
    # One fixed fight: with alpha 1 and gamma 1 the key just before the end holds the reward.
    V = day.td0_values(heuristic_policy, coarse_key, 1, alpha=1.0, gamma=1.0, seed=0)
    assert set(V.values()) <= {-1.0, 0.0, 1.0}
    assert 1.0 in V.values() or -1.0 in V.values()


def test_nstep_targets_stop_at_done_and_bootstrap_otherwise():
    r = [0, 0, 0, 1]
    v = [0.5] * 5
    d = [0, 0, 0, 1]
    assert day.nstep_targets(r, v, d, 1, 0.9).tolist() == pytest.approx([0.45, 0.45, 0.45, 1.0])
    assert day.nstep_targets(r, v, d, 2, 0.9).tolist() == pytest.approx([0.405, 0.405, 0.9, 1.0])
    assert day.nstep_targets(r, v, d, 10, 0.9).tolist() == pytest.approx([0.729, 0.81, 0.9, 1.0])
    # No done: the last targets bootstrap on values[T].
    assert day.nstep_targets([0, 0], [0.1, 0.2, 0.3], [0, 0], 1, 1.0).tolist() == pytest.approx([0.2, 0.3])
    assert day.nstep_targets([0, 0], [0.1, 0.2, 0.3], [0, 0], 5, 1.0).tolist() == pytest.approx([0.3, 0.3])


def test_legal_classes_and_class_action():
    row = np.zeros(140, dtype=bool)
    row[[8, 17, 24, 85, 128]] = True  # slot 1 untargeted, slot 2 on enemy 1, slot 3 untargeted, potion 0 on 5, end
    assert day.legal_classes(row).tolist() == [True, True, True, True]
    assert day.class_action(day.ATTACK, row) == 17
    assert day.class_action(day.SKILL, row) == 8
    assert day.class_action(day.POTION, row) == 85
    assert day.class_action(day.OTHER, row) == 128
    row[:] = False
    row[[130, 139]] = True
    assert day.legal_classes(row).tolist() == [False, False, False, True]
    assert day.class_action(day.OTHER, row) == 130


def test_real_key_on_the_first_turn():
    envs = day.make_envs(8, seed=1)
    legal = envs.legal()
    for i in range(envs.n):
        quarter, energy, danger, attacks, skills = day.real_key(envs.obs[i], legal[i])
        assert quarter == 3 or quarter == 2  # a fresh fight starts near full HP
        assert energy == 3 and danger in ("safe", "ok", "low", "lethal")
        assert 0 <= attacks <= 3 and 0 <= skills <= 3
        assert (attacks > 0) == day.legal_classes(legal[i])[day.ATTACK]
    envs.close()


def test_real_table_agent_learns_to_beat_random():
    agent = day.RealTableAgent()
    envs = day.make_envs(64, seed=0)
    wr, n = agent.train(envs, 4000, seed=0)
    envs.close()
    assert n > 5000 and 0 <= wr <= 1
    assert 100 < len(agent.Q) < 2000
    greedy, fights = day.evaluate(agent.act, n_steps=1000, seed=7)
    assert fights > 1500
    assert greedy > 0.45, "random wins 0.34; the table should beat it clearly"
