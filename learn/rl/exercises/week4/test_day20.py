import json

from conftest import exercise

day = exercise("week4.day20_capstone")


def test_capstone_trains_for_a_budget_and_writes_the_report(tmp_path):
    net, history = day.train_for(25, tmp_path, log=None, n_envs=64, T=128, minibatch=1024, eval_every=5, eval_fights=256, threads=2)
    assert len(history) >= 3 and history[-1]["eval_win"] is not None
    assert all(set(h) == set(day.COLUMNS) for h in history)
    assert (tmp_path / "best.pt").exists() and json.loads((tmp_path / "history.json").read_text()) == history
    assert history[-1]["eval_win"] > 0.5 or max(h["eval_win"] or 0 for h in history) > 0.5
    out = day.report(net, history, tmp_path, n_fights=256, threads=2)
    assert set(out["policies"]) == {"random", "cards first", "slot net (best)"}
    assert out["policies"]["slot net (best)"]["win_rate"] > out["policies"]["random"]["win_rate"]
    assert out["iterations"] == len(history) and out["best_eval_win"] > 0.5
    md = (tmp_path / "report.md").read_text()
    assert md.startswith("# Capstone report") and "| random |" in md and "| encounter |" in md
    assert json.loads((tmp_path / "report.json").read_text())["steps"] == history[-1]["steps"]
