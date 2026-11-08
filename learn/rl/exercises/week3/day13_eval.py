"""Day 13: an evaluation protocol. Fixed seed, greedy actions, a fixed number of fights,
standard errors, a per-encounter table, and the number of fights it takes to separate two
policies."""

import math
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve()
while not (ROOT / "Cargo.toml").exists():
    ROOT = ROOT.parent
sys.path[:0] = [str(ROOT / "python"), str(ROOT / "learn/rl/exercises")]

import numpy as np  # noqa: E402
import torch  # noqa: E402

from common import make_envs, vocab  # noqa: E402
from week1.day2_observe import cards_first  # noqa: E402
from week2.day8_nets import DEVICE, featurise  # noqa: E402

EVAL_SEED = 7


def random_legal(obs, legal, rng=np.random.default_rng(0)):
    """A policy in the protocol's shape, `policy(obs, legal) -> actions`: a uniform legal
    action per env."""
    return np.where(legal, rng.random(legal.shape), -1.0).argmax(axis=1)


def cards_first_policy(obs, legal):
    """Day 2's cards-first rule in the protocol's shape."""
    return cards_first(legal)


def greedy(net, featurise_fn=featurise, device=DEVICE):
    """`policy(obs, legal)` that plays `net`'s argmax legal action (day 10's ActorCritic or any
    net with the same `forward(x, legal)`, fed `featurise_fn(obs, legal)`)."""
    raise NotImplementedError  # TODO


def standard_error(p, n):
    """Standard error of a win rate `p` measured over `n` fights: sqrt(p (1 - p) / n)."""
    raise NotImplementedError  # TODO


def fights_to_separate(p1, p2, z=1.96):
    """Fights per policy so that a true gap |p1 - p2| is z standard errors of the difference:
    n = z² (p1 (1 - p1) + p2 (1 - p2)) / (p1 - p2)². Infinite for p1 == p2."""
    raise NotImplementedError  # TODO


def evaluate(policy, n_fights=512, n_envs=64, seed=EVAL_SEED, threads=1):
    """Plays `policy(obs, legal) -> actions` on `make_envs(n_envs, seed)` until `n_fights`
    fights have ended (fights still running are ignored). Returns a dict: win_rate, se
    (standard_error), n, mean_length (actions per finished fight), by_encounter {name: (wins,
    fights)} keyed by the encounter name (static observation bytes 4-5, read at each fight's
    start). The same seed always yields the same starts in the same order."""
    raise NotImplementedError  # TODO


def compare(a, b):
    """Two `evaluate` results side by side. Returns (diff, se_diff, rows): b's win rate minus
    a's with the standard error of the difference, and one row per encounter both saw, (name,
    win rate a, win rate b, fights in a, fights in b), sorted by the gap, largest first."""
    raise NotImplementedError  # TODO


if __name__ == "__main__":
    from week2.day10_ppo import train

    print(f"fights to tell 0.60 from 0.65 at 95%: {fights_to_separate(0.60, 0.65)}; 0.60 from 0.70: {fights_to_separate(0.60, 0.70)}")
    results = {"random": evaluate(random_legal), "cards first": evaluate(cards_first_policy)}
    net, _ = train(iterations=15)
    results["ppo (15 it)"] = evaluate(greedy(net))
    for name, r in results.items():
        print(f"{name:12s} win {r['win_rate']:.3f} ± {r['se']:.3f}  length {r['mean_length']:.1f}  ({r['n']} fights)")
    diff, se, rows = compare(results["cards first"], results["ppo (15 it)"])
    print(f"ppo - cards first: {diff:+.3f} ± {se:.3f}")
    for name, pa, pb, na, nb in rows[:5] + rows[-3:]:
        print(f"  {name:34s} {pa:.2f} -> {pb:.2f}  ({na}/{nb} fights)")
