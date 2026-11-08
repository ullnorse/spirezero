"""Day 18: debugging training. The signatures of the usual failures in a logged history, and a
small sweep on fixed seeds picked by the evaluation protocol.

Run:  .venv/bin/python learn/rl/exercises/week4/day18_debug.py
"""

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve()
while not (ROOT / "Cargo.toml").exists():
    ROOT = ROOT.parent
sys.path[:0] = [str(ROOT / "python"), str(ROOT / "learn/rl/exercises")]

import numpy as np  # noqa: E402

PROBLEMS = ("no learning", "entropy collapse", "policy jumping", "steps too small", "value exploding", "stalling episodes")


def diagnose(history):
    """Problems visible in `history` (a list of per-iteration dicts with win_rate, entropy,
    clipfrac, approx_kl, value_loss and optionally mean_length), as a list of (problem,
    evidence string) in PROBLEMS order, empty for a healthy run. With fewer than 4 rows, only
    "policy jumping" and "stalling episodes" can be reported. Averages are over the last
    quarter of the rows (at least one row) against the first quarter.
    - no learning: the late win rate is within 0.03 of the early one and the late entropy
      within 10% of the early one;
    - entropy collapse: the late entropy is below 15% of the early one and the win rate did
      not rise by more than 0.03 between the early and late quarters;
    - policy jumping: any row with approx_kl above 0.05 or clipfrac above 0.5;
    - steps too small: the mean clipfrac is below 0.02 and the mean |approx_kl| below 1e-3;
    - value exploding: the late value_loss is above 3 × the early one and above 1.0;
    - stalling episodes: any mean_length above 500."""
    raise NotImplementedError  # TODO


def synthetic(kind, n=40, seed=0):
    """A made-up history that shows `kind` ("healthy" or one of PROBLEMS), for testing
    `diagnose`: win rate climbing from 0.34 to 0.75 with entropy falling from 3.0 to 1.2
    unless the problem says otherwise; clipfrac 0.2, approx_kl 0.01, value_loss 0.5,
    mean_length 30 unless the problem says otherwise."""
    rng = np.random.default_rng(seed)
    t = np.linspace(0, 1, n)
    win = 0.34 + 0.41 * (1 - np.exp(-4 * t))
    ent = 3.0 - 1.8 * (1 - np.exp(-3 * t))
    clip, kl, vl, length = np.full(n, 0.2), np.full(n, 0.01), np.full(n, 0.5), np.full(n, 30.0)
    if kind == "no learning":
        win, ent = np.full(n, 0.34), np.full(n, 3.0)
    elif kind == "entropy collapse":
        ent = 3.0 * np.exp(-12 * t)
        win = np.where(t < 0.1, 0.34 + 0.6 * t, 0.40)
    elif kind == "policy jumping":
        kl[n // 2] = 0.2
        clip[n // 2] = 0.7
    elif kind == "steps too small":
        clip, kl = np.full(n, 0.005), np.full(n, 2e-4)
        win, ent = 0.34 + 0.12 * t, 3.0 - 0.4 * t
    elif kind == "value exploding":
        vl = 0.5 * np.exp(4 * t)
    elif kind == "stalling episodes":
        length = 30 + 2000 * t
    elif kind != "healthy":
        raise ValueError(kind)
    noise = 0.005 * rng.standard_normal(n)
    return [{"win_rate": float(win[i] + noise[i]), "entropy": float(ent[i]), "clipfrac": float(clip[i]), "approx_kl": float(kl[i]),
             "value_loss": float(vl[i]), "mean_length": float(length[i])} for i in range(n)]


def sweep(configs, seeds, train_fn, eval_fn):
    """For every config (a dict of keyword arguments) and seed: `net, history =
    train_fn(seed=seed, **config)`, then `score = eval_fn(net)`. Returns a list of rows
    {"config", "seed", "score", "problems": diagnose(history)} in that order."""
    raise NotImplementedError  # TODO


def pick(rows):
    """The config with the highest mean score over its seeds, as (config, mean score, standard
    error over seeds (0 with one seed)). Ties go to the first config in row order."""
    raise NotImplementedError  # TODO


if __name__ == "__main__":
    for kind in ("healthy",) + PROBLEMS:
        print(f"{kind:18s} → {diagnose(synthetic(kind)) or 'healthy'}")
    from week2.day10_ppo import train
    from week3.day13_eval import evaluate, greedy

    configs = [{"lr": 3e-4, "clip": 0.2}, {"lr": 1e-3, "clip": 0.2}, {"lr": 3e-4, "clip": 0.1}]
    rows = sweep(configs, seeds=(0, 1), train_fn=lambda seed, **c: train(iterations=20, seed=seed, **c),
                 eval_fn=lambda net: evaluate(greedy(net), n_fights=512)["win_rate"])
    for r in rows:
        print(f"{r['config']} seed {r['seed']}: eval {r['score']:.3f}  {[p for p, _ in r['problems']] or 'healthy'}")
    print("pick:", pick(rows))
