import pytest

from conftest import exercise

day = exercise("week4.day18_debug")


def test_synthetic_histories_have_the_columns():
    h = day.synthetic("healthy")
    assert len(h) == 40 and set(h[0]) >= {"win_rate", "entropy", "clipfrac", "approx_kl", "value_loss", "mean_length"}
    assert h[-1]["win_rate"] > 0.7 > 0.4 > h[0]["win_rate"]
    with pytest.raises(ValueError):
        day.synthetic("something else")


def test_diagnose_names_each_problem_and_only_that_one():
    assert day.diagnose(day.synthetic("healthy")) == []
    for kind in day.PROBLEMS:
        found = [p for p, _ in day.diagnose(day.synthetic(kind))]
        assert found == [kind], (kind, found)
    assert day.diagnose(day.synthetic("healthy")[:3]) == []
    short = day.synthetic("policy jumping", n=3)
    short[1]["approx_kl"] = 0.2
    assert [p for p, _ in day.diagnose(short)] == ["policy jumping"]


def test_diagnose_evidence_is_a_string_in_problem_order():
    h = day.synthetic("entropy collapse")
    for r in h:
        r["mean_length"] = 900.0
    found = day.diagnose(h)
    assert [p for p, _ in found] == ["entropy collapse", "stalling episodes"]
    assert all(isinstance(e, str) and e for _, e in found)


def test_sweep_and_pick_with_a_fake_trainer():
    def train_fn(seed, lr):
        return {"lr": lr, "seed": seed}, day.synthetic("healthy", seed=seed)

    def eval_fn(net):
        return net["lr"] * 100 + 0.01 * net["seed"]
    rows = day.sweep([{"lr": 0.001}, {"lr": 0.003}], seeds=(0, 1), train_fn=train_fn, eval_fn=eval_fn)
    assert [(r["config"]["lr"], r["seed"]) for r in rows] == [(0.001, 0), (0.001, 1), (0.003, 0), (0.003, 1)]
    assert all(r["problems"] == [] for r in rows)
    config, mean, se = day.pick(rows)
    assert config == {"lr": 0.003} and mean == pytest.approx(0.305) and se == pytest.approx(0.005)
    assert day.pick(rows[:1]) == ({"lr": 0.001}, pytest.approx(0.1), 0.0)


def test_sweep_runs_the_real_trainer_briefly():
    ppo = exercise("week2.day10_ppo")
    ev = exercise("week3.day13_eval")
    rows = day.sweep([{"lr": 3e-4}], seeds=(0,), train_fn=lambda seed, **c: ppo.train(iterations=3, n_envs=32, T=64, seed=seed, **c),
                     eval_fn=lambda net: ev.evaluate(ev.greedy(net), n_fights=100, n_envs=32)["win_rate"])
    assert len(rows) == 1 and 0 <= rows[0]["score"] <= 1
