import numpy as np
import pytest

from conftest import exercise

day = exercise("week1.day4_bandit")


def test_update_is_an_incremental_mean():
    b = day.Bandit(2)
    xs = [1, 0, 0, 1, 1, 1, 0]
    for x in xs:
        b.update(1, x)
    assert b.n.tolist() == [0, 7]
    assert b.q[1] == pytest.approx(np.mean(xs))
    assert b.q[0] == 0.0


def test_select_eps_is_greedy_at_zero_and_uniform_at_one():
    b = day.Bandit(3)
    b.q[:] = [0.2, 0.9, 0.5]
    rng = np.random.default_rng(0)
    assert all(b.select_eps(0.0, rng) == 1 for _ in range(20))
    picks = {b.select_eps(1.0, rng) for _ in range(200)}
    assert picks == {0, 1, 2}
    b.q[:] = [0.9, 0.9, 0.1]
    ties = {b.select_eps(0.0, rng) for _ in range(100)}
    assert ties == {0, 1}, "ties are broken at random"


def test_select_ucb_tries_everything_once_then_balances():
    b = day.Bandit(3)
    assert b.select_ucb(1.0) == 0
    for arm, r in ((0, 1), (1, 0), (2, 0)):
        b.update(arm, r)
    assert b.select_ucb(1.0) == 0
    for _ in range(50):
        b.update(0, 1)
    # Arm 0 is well known at 1.0; with a large bonus the rarely tried arms win the bound.
    assert b.select_ucb(10.0) in (1, 2)


def test_simulate_finds_the_best_arm():
    b, regret = day.simulate([0.3, 0.5, 0.7], 3000, eps=0.1, seed=0)
    assert b.n.sum() == 3000
    assert b.n[2] > 0.6 * 3000
    assert abs(b.q[2] - 0.7) < 0.08
    assert len(regret) == 3000 and (np.diff(regret) >= -1e-12).all()
    assert regret[-1] < 300
    _, ucb = day.simulate([0.3, 0.5, 0.7], 3000, ucb=1.0, seed=0)
    assert ucb[-1] < 200


def test_strategies_prefer_the_right_cells():
    row = np.zeros(140, dtype=bool)
    row[[8, 17, 24, 128]] = True  # slot 1 untargeted, slot 2 on enemy 1, slot 3 untargeted, end
    assert day.strat_attacks_first(row, None) == 17
    assert day.strat_skills_first(row, None) == 8
    row[:] = False
    row[[128, 130]] = True
    assert day.strat_attacks_first(row, None) == 128
    assert day.strat_skills_first(row, None) == 128


def test_play_fight_runs_one_fight_to_its_end():
    envs = day.make_envs(1, seed=0)
    rng = np.random.default_rng(0)
    for strat in day.STRATEGIES.values():
        r = day.play_fight(envs, strat, rng)
        assert r in (0.0, 1.0)
        assert envs.done[0] == 1
    envs.close()


def test_run_real_pulls_every_strategy_and_counts():
    b, names = day.run_real(pulls=60, eps=0.3, seed=0)
    assert names == list(day.STRATEGIES)
    assert b.n.sum() == 60
    assert ((b.q >= 0) & (b.q <= 1)).all()
    assert (b.n > 0).sum() >= 3
