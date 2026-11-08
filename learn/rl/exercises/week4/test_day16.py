import numpy as np
import pytest

from conftest import exercise

day = exercise("week4.day16_runenv")


def test_layout_matches_the_engine():
    envs = day.make_run_envs(2)
    L = envs.layout
    assert int(L.run_base) == day.RUN_BASE and int(L.actions) == day.RUN_ACTIONS
    assert envs.legal().shape == (2, day.RUN_ACTIONS)
    envs.close()


def test_run_header_decodes_a_fresh_run():
    envs = day.make_run_envs(4)
    h = day.run_header(envs.obs[0])
    assert h["character"] == 4 and h["ascension"] == 0 and h["phase"] in day.PHASES
    assert 0 < h["hp"] <= h["max_hp"] and h["gold"] >= 0 and h["deck_size"] >= 10 and h["floor"] <= 1
    assert h["map_col"] in (255, *range(7)) and h["max_energy"] == 3
    envs.close()


def test_decode_run_action_covers_every_block():
    assert day.decode_run_action(5) == ("combat", 5)
    assert day.decode_run_action(day.NODE_BASE + 3) == ("node", 3)
    assert day.decode_run_action(day.PICK_CARD_BASE + 7) == ("pick_card", (1, 2))
    assert day.decode_run_action(day.ALTERNATIVE_BASE + 3) == ("alternative", (1, 2))
    assert day.decode_run_action(day.PROCEED) == ("proceed", 0)
    assert day.decode_run_action(day.CONFIRM_SELECT) == ("confirm", 0)
    kinds = {day.decode_run_action(i)[0] for i in range(day.RUN_ACTIONS)}
    assert len(kinds) == 20
    with pytest.raises(ValueError):
        day.decode_run_action(day.RUN_ACTIONS)


def test_policies_only_pick_legal_run_level_actions():
    envs = day.make_run_envs(8)
    for _ in range(30):
        legal = envs.legal()
        for policy in (day.random_run_policy, day.lowest_policy, day.prefer_policy([(day.SKIP_BASE, 8), (day.PROCEED, 1)])):
            a = policy(envs.obs, legal)
            assert a.shape == (8,) and legal[np.arange(8), a].all() and (a >= day.RUN_BASE).all()
        envs.step(day.lowest_policy(envs.obs, legal))
    envs.close()


def test_measure_floors_is_paired_on_seeds_and_lowest_beats_random():
    r = day.measure_floors(day.random_run_policy, n_runs=120, n_envs=32)
    low = day.measure_floors(day.lowest_policy, n_runs=120, n_envs=32)
    again = day.measure_floors(day.lowest_policy, n_runs=120, n_envs=32)
    assert low[2] == again[2] and len(low[2]) == 120 and sum(low[3].values()) == 120
    assert all(f >= 1 for f in r[2]) and r[0] > 5
    assert low[0] > r[0] + 1.5 * (low[1] + r[1])
