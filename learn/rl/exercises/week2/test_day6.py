import numpy as np
import pytest

from conftest import exercise

day = exercise("week2.day6_qlearn")
from common.minifight import DEFEND, END, STRIKE, MiniFight, State, heuristic_policy, win_rate  # noqa: E402


def test_coarse_key_buckets_danger_hp_and_strikes_to_kill():
    s = State(player_hp=10, enemy_hp=12, strength=8, intent="chomp", strikes=3, defends=2)
    assert day.coarse_key(s) == ("chomp", 3, 3, 2, "lethal", "hp2", 2)
    s = State(player_hp=28, block=12, enemy_hp=48, intent="chomp", strikes=2, defends=3)
    assert day.coarse_key(s) == ("chomp", 3, 2, 3, "safe", "hp3", 6)
    s = State(player_hp=9, enemy_hp=1, enemy_block=6, intent="bellow", strikes=1, defends=1, energy=1)
    assert day.coarse_key(s) == ("bellow", 1, 1, 1, "safe", "hp1", 2)
    s = State(player_hp=20, intent="thrash", strikes=0, defends=5)
    assert day.coarse_key(s) == ("thrash", 3, 0, 5, "ok", "hp3", 6)


def test_update_moves_q_toward_the_target():
    a = day.QAgent(alpha=0.5, gamma=0.9)
    a.update("k1", STRIKE, 0.0, False, "k2", (True, True, True))
    assert a.q("k1").tolist() == [0.0, 0.0, 0.0]
    a.q("k2")[:] = [0.2, 0.8, -1.0]
    a.update("k1", STRIKE, 0.0, False, "k2", (True, True, True))
    assert a.q("k1")[STRIKE] == pytest.approx(0.5 * 0.9 * 0.8)
    a.update("k1", DEFEND, 0.0, False, "k2", (True, False, True))  # Defend illegal next: max over Strike, End
    assert a.q("k1")[DEFEND] == pytest.approx(0.5 * 0.9 * 0.2)
    a.update("k1", END, -1.0, True, "k2", (True, True, True))  # terminal: no bootstrap
    assert a.q("k1")[END] == pytest.approx(-0.5)


def test_greedy_respects_the_mask_and_breaks_ties_low():
    a = day.QAgent()
    s = State(strikes=2, defends=3)
    a.q(a.key(s))[:] = [0.1, 0.9, 0.5]
    assert a.greedy(s, (True, True, True)) == DEFEND
    assert a.greedy(s, (True, False, True)) == END
    assert a.greedy(State(strikes=1, defends=1), (True, True, True)) == STRIKE


def test_act_is_greedy_at_eps_zero_and_explores_at_one():
    a = day.QAgent(eps=0.0)
    s = State(strikes=2, defends=3)
    a.q(a.key(s))[:] = [0.1, 0.9, 0.5]
    assert all(a.act(s, (True, True, True)) == DEFEND for _ in range(20))
    a.eps = 1.0
    assert {a.act(s, (True, True, True)) for _ in range(200)} == {STRIKE, DEFEND, END}


def test_episode_learns_the_terminal_steps_first():
    a = day.QAgent(alpha=0.5)
    s = a.episode()
    assert s.done and a.episodes == 1 and a.steps == s.steps
    assert any((row != 0).any() for row in a.Q.values()), "the terminal update is non-zero"


def test_train_beats_the_hand_written_rule_with_the_full_key():
    a = day.QAgent(day.full_key, seed=1)
    curve = day.train(a, 20000)
    assert len(curve) == 40 and 0 <= min(curve) <= max(curve) <= 1
    assert curve[-1] > curve[0] - 0.1
    assert win_rate(a.policy, 2000, seed=5) > 0.85  # the rule gets 0.80
    assert len(a.Q) > 5000


def test_coarse_table_is_small_and_nearly_as_good():
    a = day.QAgent(day.coarse_key, seed=1)
    day.train(a, 20000)
    assert len(a.Q) < 3000
    assert win_rate(a.policy, 2000, seed=5) > 0.7
    b = day.QAgent(day.blind_key, seed=1)
    day.train(b, 5000)
    assert win_rate(b.policy, 1000, seed=5) < 0.3, "without the enemy in the key there is nothing to learn"
