import numpy as np

from conftest import exercise

day = exercise("week4.day19_engine")
cards_first = exercise("week1.day2_observe").cards_first


def header(hp=50, max_hp=80, block=0, energy=3, max_energy=3, hand=5, status=0):
    o = np.zeros((1, 972), dtype=np.uint8)
    o[0, :10] = np.array([hp, max_hp, block, energy, max_energy], dtype="<i2").view(np.uint8)
    o[0, 16] = hand
    o[0, 23] = status
    return o


def test_check_header_catches_each_violation():
    assert day.check_header(header()) == []
    assert len(day.check_header(header(hp=81))) == 1
    assert len(day.check_header(header(hp=-1))) == 1
    assert len(day.check_header(header(block=-3))) == 1
    assert len(day.check_header(header(hand=11))) == 1
    assert len(day.check_header(header(status=7))) == 1
    assert len(day.check_header(np.concatenate([header(hp=81), header(hand=11)]))) == 2


def test_check_mask_catches_each_violation():
    legal = np.zeros((1, 140), dtype=bool)
    legal[0, day.END_TURN] = True
    assert day.check_mask(header(hand=0), legal) == []
    legal[0, 3 * 8 + 1] = True  # slot 3 with 3 cards in hand
    assert len(day.check_mask(header(hand=3), legal)) == 1 and day.check_mask(header(hand=4), legal) == []
    legal[0, day.DONE] = True
    assert len(day.check_mask(header(hand=4), legal)) == 1
    assert len(day.check_mask(header(), np.zeros((1, 140), dtype=bool))) == 1
    assert len(day.check_mask(header(), np.ones((1, 141), dtype=bool))) == 1


def test_check_stats():
    ok = {"actions": 4, "unmapped": 0, "ended": 2, "won": 1, "capped": 0, "reset": 2}
    assert day.check_stats(ok, 4, [1, 0, 1, 0], [1, 0, 0, 0]) == []
    assert len(day.check_stats({**ok, "actions": 3}, 4, [1, 0, 1, 0], [1, 0, 0, 0])) == 1
    assert len(day.check_stats({**ok, "won": 2}, 4, [1, 0, 1, 0], [1, 0, 0, 0])) == 1
    assert len(day.check_stats({**ok, "reset": 1}, 4, [1, 0, 1, 0], [1, 0, 0, 0])) == 1
    assert len(day.check_stats(ok, 4, [1, 1, 1, 0], [1, 0, 0, 0])) == 1


def test_the_real_env_keeps_its_promises():
    violations, ended, capped = day.audit(cards_first, n_envs=32, n_steps=400, seed=2)
    assert violations == [] and ended > 100 and capped == 0
    random = exercise("week3.day11_experiment").random_legal
    violations, ended, capped = day.audit(random, n_envs=32, n_steps=400, seed=2, threads=4)
    assert violations == [] and ended > 100


def test_illegal_actions_are_rejected_and_nothing_is_applied():
    envs = day.make_envs(4, seed=1)
    msg, unchanged = day.illegal_is_rejected(envs)
    assert msg and "legal" in msg.lower() and unchanged
    envs.step(envs.legal().argmax(axis=1))  # still usable afterwards
    envs.close()


def test_bench_reports_every_thread_count():
    out = day.bench(threads_list=(1, 2), n_envs=32, n_steps=50)
    assert set(out) == {1, 2} and all(v > 1000 for v in out.values())
