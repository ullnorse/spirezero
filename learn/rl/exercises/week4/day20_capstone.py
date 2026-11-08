"""Day 20, the capstone: train the slot net with PPO on real Act 1 fights for a time budget,
evaluate it by the protocol against random and cards first, and write the report. Reference
solution.

Run:  .venv/bin/python learn/rl/exercises/week4/day20_capstone.py [minutes] [out_dir]
      five minutes on an RTX 4080 Super is the intended budget; the report lands in out_dir.
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
from week2.day8_nets import DEVICE  # noqa: E402
from week2.day10_ppo import update  # noqa: E402
from week3.day11_experiment import COLUMNS, format_row  # noqa: E402
from week3.day13_eval import cards_first_policy, compare, evaluate, greedy, random_legal  # noqa: E402
from week3.day14_features import SlotNet, featurise14, rollout  # noqa: E402

DEFAULTS = dict(n_envs=256, T=128, lr=3e-4, epochs=4, minibatch=4096, ent_coef=0.01, clip=0.2, seed=0, eval_every=10,
                eval_fights=1024, threads=4)


def train_for(budget_s, out_dir, log=print, **overrides):
    """PPO with the slot net for `budget_s` seconds of wall time (the last iteration may run
    over). Settings are DEFAULTS updated by `overrides`. Every `eval_every` iterations and
    after the last, `eval_win` by day 13's `evaluate` with `greedy(net, featurise14)` over
    `eval_fights` fights, and `best.pt` (state_dict + the settings + it + eval_win) is saved
    when it improves. Rows as day 11's COLUMNS with `steps` the env steps so far; the history
    goes to `history.json` in `out_dir`. Returns (best net: a SlotNet loaded from best.pt,
    history)."""
    raise NotImplementedError  # TODO


def report(net, history, out_dir, n_fights=1024, threads=4):
    """Evaluates random, cards first and the net by day 13's protocol over `n_fights` each,
    writes `report.json` ({"policies": {name: evaluate result}, "steps", "iterations",
    "best_eval_win", "encounters": compare(cards first, net)'s rows}) and `report.md` (a
    table of the three policies: win rate ± se, mean length; then the per-encounter table of
    cards first against the net, sorted by gain) to `out_dir`. Returns the dict."""
    raise NotImplementedError  # TODO


def main(argv):
    minutes = float(argv[1]) if len(argv) > 1 else 5.0
    out_dir = pathlib.Path(argv[2]) if len(argv) > 2 else ROOT / "target/learn/capstone"
    net, history = train_for(minutes * 60, out_dir)
    out = report(net, history, out_dir)
    for name, r in out["policies"].items():
        print(f"{name:16s} win {r['win_rate']:.3f} ± {r['se']:.3f}  length {r['mean_length']:.1f}")
    print(f"report: {out_dir / 'report.md'}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
