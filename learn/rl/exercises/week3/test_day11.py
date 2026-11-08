import json

import torch

from conftest import exercise

day = exercise("week3.day11_experiment")


def test_measure_counts_fights_and_lengths():
    wr, n, length = day.measure(day.random_legal, n_envs=16, n_steps=300, seed=1)
    assert n > 50 and 0.1 < wr < 0.6 and 5 < length < 100
    a = day.measure(day.cards_first, n_envs=16, n_steps=300, seed=1)
    assert a == day.measure(day.cards_first, n_envs=16, n_steps=300, seed=1) and a[0] > wr
    b = day.baselines(n_envs=32, n_steps=400)
    assert set(b) == {"random", "cards_first"} and b["cards_first"][0] > b["random"][0]


def test_checkpoint_round_trips_to_the_same_policy(tmp_path):
    net = day.ActorCritic().to(day.DEVICE)
    day.save_checkpoint(net, tmp_path / "a.pt", {"it": 3, "eval_win": 0.5})
    loaded, meta = day.load_checkpoint(tmp_path / "a.pt")
    assert meta == {"it": 3, "eval_win": 0.5} and not loaded.training
    x = torch.randn(5, net.body[0].in_features, device=day.DEVICE)
    legal = torch.ones(5, 140, dtype=torch.bool, device=day.DEVICE)
    with torch.no_grad():
        assert torch.allclose(net(x, legal)[0].logits, loaded(x, legal)[0].logits)


def test_format_row_prints_every_column():
    row = {"it": 3, "steps": 8192, "win_rate": 0.5, "eval_win": None, "entropy": 2.345, "clipfrac": 0.12, "approx_kl": 0.01234,
           "value_loss": 0.5, "mean_length": 23.4, "sps": 12345.6}
    line = day.format_row(row)
    assert line.startswith("it=3  steps=8192  win_rate=0.500  eval_win=-  entropy=2.35")
    assert "approx_kl=0.0123" in line and line.endswith("sps=12346")
    assert all(f"{k}=" in line for k in day.COLUMNS)


def test_experiment_keeps_history_and_checkpoints(tmp_path):
    net, history = day.experiment(tmp_path, iterations=4, eval_every=2, n_envs=32, T=64, log=None)
    assert len(history) == 4 and [h["eval_win"] is not None for h in history] == [False, True, False, True]
    assert history[-1]["steps"] == 4 * 32 * 64 and all(h["sps"] > 0 and h["mean_length"] > 1 for h in history)
    saved = json.loads((tmp_path / "history.json").read_text())
    assert saved == history
    last, meta = day.load_checkpoint(tmp_path / "last.pt")
    assert meta["it"] == 3 and meta["eval_win"] == history[-1]["eval_win"]
    best, bmeta = day.load_checkpoint(tmp_path / "best.pt")
    assert bmeta["eval_win"] == max(h["eval_win"] for h in history if h["eval_win"] is not None)
