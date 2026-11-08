"""Day 4: bandits. ε-greedy and UCB on simulated arms, then a bandit over fixed strategies on
real fights.

Run:  .venv/bin/python learn/rl/exercises/week1/day4_bandit.py
Test: .venv/bin/python -m pytest learn/rl/exercises/week1/test_day4.py
"""

import math  # noqa: F401
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve()
while not (ROOT / "Cargo.toml").exists():
    ROOT = ROOT.parent
sys.path[:0] = [str(ROOT / "python"), str(ROOT / "learn/rl/exercises")]

import numpy as np  # noqa: E402

from common import END_TURN, POTION_BASE, TARGET_SLOTS, make_envs  # noqa: E402


class Bandit:
    """Sample-mean estimates for `n_arms` arms. `optimistic` is the estimate an untried arm
    starts with (0 for honest, 1 for optimistic)."""

    def __init__(self, n_arms, optimistic=0.0):
        self.n = np.zeros(n_arms, dtype=int)       # pulls per arm
        self.q = np.full(n_arms, float(optimistic))  # estimated win rate per arm

    def update(self, arm, reward):
        """Count the pull, then move the estimate toward the reward by 1/n of the gap."""
        raise NotImplementedError  # TODO

    def select_eps(self, eps, rng):
        """With probability `eps` a uniform arm; otherwise the arm with the largest estimate,
        ties broken at random (`rng` is a numpy Generator)."""
        raise NotImplementedError  # TODO

    def select_ucb(self, c):
        """Upper confidence bound: an untried arm first (lowest index), then the argmax of
        q + c * sqrt(ln(total pulls) / n)."""
        raise NotImplementedError  # TODO


def simulate(probs, pulls, eps=0.1, optimistic=0.0, ucb=None, seed=0):
    """A Bernoulli bandit with win probabilities `probs`. Returns (bandit, regret curve):
    regret after each pull is the cumulative gap between the best arm's probability and the
    pulled arm's. `ucb` (a float c) switches from ε-greedy to UCB. Draw the reward as
    `rng.random() < probs[arm]`."""
    raise NotImplementedError  # TODO


# ---- fixed strategies on the real env: strategy(legal_row, rng) -> action index -----------
def _plays(legal_row):
    """The legal play and potion indices (below POTION_BASE are plays; here: everything below
    END_TURN counts as a card)."""
    return np.flatnonzero(legal_row[:POTION_BASE])


def strat_random(legal_row, rng):
    return int(rng.choice(np.flatnonzero(legal_row)))


def strat_cards_first(legal_row, rng):
    plays = _plays(legal_row)
    return int(plays[0]) if len(plays) else int(np.flatnonzero(legal_row)[0])


def strat_attacks_first(legal_row, rng):
    """Prefer plays on an enemy (target ≥ 1): attacks. Then any play. Then end/pick."""
    raise NotImplementedError  # TODO


def strat_skills_first(legal_row, rng):
    """Prefer untargeted plays (target 0): skills and powers. Then attacks. Then end/pick."""
    raise NotImplementedError  # TODO


def strat_end_turn(legal_row, rng):
    """Ends the turn whenever it may. Never plays a card."""
    return END_TURN if legal_row[END_TURN] else int(np.flatnonzero(legal_row)[0])


STRATEGIES = dict(random=strat_random, cards_first=strat_cards_first, attacks_first=strat_attacks_first,
                  skills_first=strat_skills_first, end_turn=strat_end_turn)


def play_fight(envs, strategy, rng):
    """Plays one fight to the end in a 1-env batch with `strategy`. Returns 1.0 for a win."""
    raise NotImplementedError  # TODO


def run_real(pulls=300, eps=0.1, optimistic=0.0, seed=0):
    """ε-greedy over STRATEGIES, one real fight per pull (`make_envs(1, seed=seed)`).
    Returns (bandit, list of strategy names in arm order)."""
    raise NotImplementedError  # TODO


if __name__ == "__main__":
    probs = [0.3, 0.5, 0.55, 0.7]
    for label, kw in (("eps 0.1", dict(eps=0.1)), ("eps 0", dict(eps=0.0)),
                      ("eps 0 optimistic", dict(eps=0.0, optimistic=1.0)), ("UCB c=1", dict(ucb=1.0))):
        b, regret = simulate(probs, 3000, **kw)
        print(f"{label:18s} pulls {b.n.tolist()}  estimates {np.round(b.q, 2).tolist()}  regret {regret[-1]:.0f}")
    b, names = run_real(600)
    for i, name in enumerate(names):
        print(f"{name:14s} pulled {b.n[i]:4d}  win rate {b.q[i]:.3f}")
