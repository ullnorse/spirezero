"""Day 11: the first experiment as a script. Baselines on a fixed seed, a log row per
iteration, checkpoints that reload to the same policy, and a run that keeps its history.


Run:  .venv/bin/python learn/rl/exercises/week3/day11_experiment.py [iterations] [out_dir]
"""

import json
import pathlib
import sys
import time

ROOT = pathlib.Path(__file__).resolve()
while not (ROOT / "Cargo.toml").exists():
    ROOT = ROOT.parent
sys.path[:0] = [str(ROOT / "python"), str(ROOT / "learn/rl/exercises")]

import numpy as np  # noqa: E402
import torch  # noqa: E402

from common import make_envs  # noqa: E402
from week1.day2_observe import cards_first  # noqa: E402
from week2.day10_ppo import DEVICE, ActorCritic, evaluate, rollout, update  # noqa: E402

EVAL_SEED = 7
COLUMNS = ("it", "steps", "win_rate", "eval_win", "entropy", "clipfrac", "approx_kl", "value_loss", "mean_length", "sps")


def random_legal(legal, rng=np.random.default_rng(0)):
    """A uniform legal action per env (`policy(legal) -> actions`, day 2's shape)."""
    return np.where(legal, rng.random(legal.shape), -1.0).argmax(axis=1)


def measure(policy, n_envs=64, n_steps=1000, seed=EVAL_SEED):
    """Win rate and mean fight length (actions) of `policy(legal) -> actions` over the fights
    that end within `n_steps` steps of `make_envs(n_envs, seed)`. Returns (win_rate, fights,
    mean_length)."""
    raise NotImplementedError  # TODO


def baselines(n_envs=64, n_steps=1000, seed=EVAL_SEED):
    """{"random": (win_rate, fights, mean_length), "cards_first": ...} by `measure`."""
    raise NotImplementedError  # TODO


def save_checkpoint(net, path, meta):
    """Writes `net.state_dict()` and `meta` (a JSON-able dict: iteration, eval win rate, the
    hyper-parameters) to `path` with torch.save."""
    raise NotImplementedError  # TODO


def load_checkpoint(path, device=DEVICE):
    """(net, meta) from `save_checkpoint`'s file: a fresh ActorCritic in eval mode on `device`
    with the saved weights."""
    raise NotImplementedError  # TODO


def format_row(row):
    """One log line from a dict with COLUMNS' keys (missing ones print as '-'): `it` and
    `steps` as integers, rates to 3 decimals, entropy/clipfrac/value_loss/mean_length to 2,
    approx_kl to 4, sps as an integer, each as `key=value` separated by two spaces."""
    raise NotImplementedError  # TODO


def experiment(out_dir, iterations=20, eval_every=5, n_envs=64, T=128, lr=3e-4, seed=0, log=print, **kw):
    """Day 10's loop with the bookkeeping: every iteration a row (COLUMNS) in the returned
    history with `steps` the env steps so far, `mean_length` the mean length of the rollout's
    finished fights (T · n / episodes), `sps` env steps per second of the whole iteration;
    every `eval_every` iterations (and after the last) `eval_win` from day 10's `evaluate`
    on seed EVAL_SEED, and a checkpoint `last.pt`; `best.pt` whenever eval_win improves.
    Writes `history.json` (the list of rows) to `out_dir` at the end. Returns (net, history)."""
    raise NotImplementedError  # TODO


if __name__ == "__main__":
    iterations = int(sys.argv[1]) if len(sys.argv) > 1 else 40
    out = sys.argv[2] if len(sys.argv) > 2 else str(ROOT / "target/learn/day11")
    for name, (wr, n, length) in baselines().items():
        print(f"{name:12s} win {wr:.3f} over {n} fights, {length:.1f} actions each")
    net, history = experiment(out, iterations)
    best, meta = load_checkpoint(pathlib.Path(out) / "best.pt")
    print(f"best checkpoint: iteration {meta['it']}, eval win {meta['eval_win']:.3f}; history in {out}/history.json")
