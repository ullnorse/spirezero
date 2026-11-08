"""Day 4: bandits. ε-greedy and UCB on simulated arms, then a bandit over fixed strategies on
real fights. Reference solution."""

import math
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
        self.n = np.zeros(n_arms, dtype=int)
        self.q = np.full(n_arms, float(optimistic))

    def update(self, arm, reward):
        """Incremental mean: q += (reward - q) / n after counting this pull."""
        self.n[arm] += 1
        self.q[arm] += (reward - self.q[arm]) / self.n[arm]

    def select_eps(self, eps, rng):
        """With probability `eps` a uniform arm; otherwise the arm with the largest estimate,
        ties broken at random."""
        if rng.random() < eps:
            return int(rng.integers(len(self.q)))
        best = np.flatnonzero(self.q == self.q.max())
        return int(rng.choice(best))

    def select_ucb(self, c):
        """Upper confidence bound: untried arms first, then argmax of
        q + c * sqrt(ln(total pulls) / n)."""
        if (self.n == 0).any():
            return int(np.flatnonzero(self.n == 0)[0])
        total = self.n.sum()
        return int(np.argmax(self.q + c * np.sqrt(math.log(total) / self.n)))


def simulate(probs, pulls, eps=0.1, optimistic=0.0, ucb=None, seed=0):
    """A Bernoulli bandit with win probabilities `probs`. Returns (bandit, regret curve):
    regret after each pull is the cumulative gap between the best arm's probability and the
    pulled arm's. `ucb` (a float c) switches from ε-greedy to UCB."""
    rng = np.random.default_rng(seed)
    probs = np.asarray(probs, dtype=float)
    b = Bandit(len(probs), optimistic)
    regret = np.zeros(pulls)
    total = 0.0
    for t in range(pulls):
        arm = b.select_ucb(ucb) if ucb is not None else b.select_eps(eps, rng)
        b.update(arm, float(rng.random() < probs[arm]))
        total += probs.max() - probs[arm]
        regret[t] = total
    return b, regret


# ---- fixed strategies on the real env: policy(legal_row, rng) -> action index -------------
def _plays(legal_row):
    return np.flatnonzero(legal_row[:POTION_BASE])


def strat_random(legal_row, rng):
    return int(rng.choice(np.flatnonzero(legal_row)))


def strat_cards_first(legal_row, rng):
    plays = _plays(legal_row)
    return int(plays[0]) if len(plays) else int(np.flatnonzero(legal_row)[0])


def strat_attacks_first(legal_row, rng):
    """Prefer plays on an enemy (target ≥ 1): attacks. Then any play. Then end/pick."""
    plays = _plays(legal_row)
    targeted = plays[plays % TARGET_SLOTS >= 1]
    if len(targeted):
        return int(targeted[0])
    return int(plays[0]) if len(plays) else int(np.flatnonzero(legal_row)[0])


def strat_skills_first(legal_row, rng):
    """Prefer untargeted plays (target 0): skills and powers. Then attacks. Then end/pick."""
    plays = _plays(legal_row)
    untargeted = plays[plays % TARGET_SLOTS == 0]
    if len(untargeted):
        return int(untargeted[0])
    return int(plays[0]) if len(plays) else int(np.flatnonzero(legal_row)[0])


def strat_end_turn(legal_row, rng):
    """Ends the turn whenever it may. Never plays a card."""
    return END_TURN if legal_row[END_TURN] else int(np.flatnonzero(legal_row)[0])


STRATEGIES = dict(random=strat_random, cards_first=strat_cards_first, attacks_first=strat_attacks_first,
                  skills_first=strat_skills_first, end_turn=strat_end_turn)


def play_fight(envs, strategy, rng):
    """Plays one fight to the end in a 1-env batch with `strategy`. Returns 1.0 for a win."""
    while True:
        envs.step(np.array([strategy(envs.legal()[0], rng)]))
        if envs.done[0]:
            return float(envs.won[0])


def run_real(pulls=300, eps=0.1, optimistic=0.0, seed=0):
    """ε-greedy over STRATEGIES, one real fight per pull. Returns (bandit, names)."""
    envs = make_envs(1, seed=seed)
    rng = np.random.default_rng(seed)
    names = list(STRATEGIES)
    b = Bandit(len(names), optimistic)
    for _ in range(pulls):
        arm = b.select_eps(eps, rng)
        b.update(arm, play_fight(envs, STRATEGIES[names[arm]], rng))
    envs.close()
    return b, names


if __name__ == "__main__":
    probs = [0.3, 0.5, 0.55, 0.7]
    for label, kw in (("eps 0.1", dict(eps=0.1)), ("eps 0", dict(eps=0.0)),
                      ("eps 0 optimistic", dict(eps=0.0, optimistic=1.0)), ("UCB c=1", dict(ucb=1.0))):
        b, regret = simulate(probs, 3000, **kw)
        print(f"{label:18s} pulls {b.n.tolist()}  estimates {np.round(b.q, 2).tolist()}  regret {regret[-1]:.0f}")
    b, names = run_real(600)
    for i, name in enumerate(names):
        print(f"{name:14s} pulled {b.n[i]:4d}  win rate {b.q[i]:.3f}")
