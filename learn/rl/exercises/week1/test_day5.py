import pytest

from conftest import exercise

day = exercise("week1.day5_value")
from common.minifight import DEFEND, STRIKE, State, heuristic_policy  # noqa: E402

# HP 10, Worm 12 with Strength 8, Chomp (20) incoming, 3 Strikes 2 Defends in hand.
S = State(player_hp=10, enemy_hp=12, strength=8, intent="chomp", strikes=3, defends=2,
          draw_s=2, draw_d=3, turn=2)


def test_mc_value_of_a_fresh_fight_matches_the_win_rate():
    v = day.mc_value(State(strikes=3, defends=2, draw_s=2, draw_d=3), heuristic_policy, 2000, seed=1)
    assert abs(v - (2 * 0.80 - 1)) < 0.12


def test_mc_value_does_not_mutate_the_state():
    s = S.copy()
    day.mc_value(s, heuristic_policy, 50)
    assert s == S


def test_mc_q_reveals_the_lethal_line():
    q = day.mc_q(S, heuristic_policy, 500)
    assert q[STRIKE] > 0.9  # two Strikes kill a 12 HP Worm
    assert q[DEFEND] < -0.9  # 10 block does not stop a 20 Chomp
    assert q[2] == pytest.approx(-1.0)


def test_mc_q_marks_illegal_actions():
    q = day.mc_q(State(energy=0, strikes=2, defends=1, draw_s=3, draw_d=4), heuristic_policy, 20)
    assert q[STRIKE] is None and q[DEFEND] is None and q[2] is not None


def test_greedy_on_q_improves_the_rule():
    assert day.greedy_vs_policy(S, heuristic_policy, 500) == (DEFEND, STRIKE)


def test_value_by_encounter_groups_outcomes_by_start():
    cards_first = exercise("week1.day2_observe").cards_first
    table = day.value_by_encounter(cards_first, n_envs=64, n_steps=1500, seed=0)
    assert len(table) >= 20
    for name, (v, n, se) in table.items():
        assert -1 <= v <= 1 and n > 0 and se >= 0
    assert table["CEREMONIAL_BEAST_BOSS"][0] < -0.8
    assert table["SLIMES_WEAK"][0] > 0.8
    assert sum(n for _, n, _ in table.values()) > 2000
