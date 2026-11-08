"""Day 7: bootstrapping. TD(0) against Monte Carlo on the mini fight, n-step targets, then a
tabular agent on the real env over action classes.

Run:  .venv/bin/python learn/rl/exercises/week2/day7_td.py
Test: .venv/bin/python -m pytest learn/rl/exercises/week2/test_day7.py
"""

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve()
while not (ROOT / "Cargo.toml").exists():
    ROOT = ROOT.parent
sys.path[:0] = [str(ROOT / "python"), str(ROOT / "learn/rl/exercises")]

import numpy as np  # noqa: E402

from common import END_TURN, POTION_BASE, TARGET_SLOTS, make_envs  # noqa: E402
from common.minifight import MiniFight, heuristic_policy  # noqa: E402
from week1.day2_observe import header  # noqa: E402
from week2.day6_qlearn import coarse_key  # noqa: E402


# ---- prediction on the mini fight ------------------------------------------------------------
def mc_values(policy, key, episodes, seed=0):
    """Every-visit Monte Carlo: V[key] is the mean return (±1) over the visits to that key.
    Returns (V dict, visit counts dict)."""
    raise NotImplementedError  # TODO


def td0_values(policy, key, episodes, alpha=0.05, gamma=1.0, seed=0):
    """TD(0): after every step V[k] += alpha * (r + gamma * V[k'] - V[k]), with V[k'] = 0 when
    the step ended the fight. Unseen keys start at 0. Returns the V dict."""
    raise NotImplementedError  # TODO


def start_value(V, policy, key, n=2000, seed=0):
    """Mean of V over the keys of `n` fresh starting states (the hand is random, so the start
    is a distribution over keys)."""
    fight = MiniFight(seed)
    return float(np.mean([V.get(key(fight.reset()), 0.0) for _ in range(n)]))


def nstep_targets(rewards, values, done, n, gamma=1.0):
    """n-step TD targets for one env's T steps. `rewards[t]`, `done[t]` describe step t;
    `values[t]` is V(s_t) with `values[T]` the value after the last step. The target at t sums
    gamma^k r_{t+k} for k < n, stopping early at a done, and adds gamma^n V(s_{t+n}) if the
    n steps ran out without a done. Returns an array of T targets."""
    raise NotImplementedError  # TODO


# ---- the real env: a table over coarse states and action classes ---------------------------
ATTACK, SKILL, POTION, OTHER = 0, 1, 2, 3
CLASSES = ("attack", "skill", "potion", "other")


def legal_classes(legal_row):
    """Which of the 4 classes have a legal index: attack = a play with target ≥ 1, skill = a
    play with target 0, potion = any potion index, other = End turn, a pick or Done."""
    raise NotImplementedError  # TODO


def class_action(cls, legal_row):
    """The lowest legal index of class `cls` (the class must be legal)."""
    raise NotImplementedError  # TODO


def incoming_damage(obs_row):
    """Sum over intent slots of damage × hits for attack intents (kind byte 1 at INTENTS_AT)."""
    from common import INTENT_BYTES, INTENTS, INTENTS_AT
    b = np.asarray(obs_row, dtype=np.uint8)
    total = 0
    for i in range(INTENTS):
        at = INTENTS_AT + i * INTENT_BYTES
        if b[at] == 1:
            total += int(b[at + 1:at + 3].copy().view("<i2")[0]) * int(b[at + 3])
    return total


def real_key(obs_row, legal_row):
    """A coarse key for a real fight: (hp quarter 0-3, energy, danger, n attacks legal capped
    at 3, n skills legal capped at 3). danger: 'safe' if incoming ≤ block, 'lethal' if it
    kills, 'low' if it leaves ≤ 25% of max HP, else 'ok'."""
    raise NotImplementedError  # TODO


class RealTableAgent:
    """Q-learning over (real_key, class) on a batch of real fights; the reward is ±1 at the
    end of a fight. `act(obs, legal, rng)` returns one legal action index per env."""

    def __init__(self, alpha=0.1, gamma=0.99, eps=0.1):
        self.alpha, self.gamma, self.eps = alpha, gamma, eps
        self.Q = {}

    def q(self, key):
        row = self.Q.get(key)
        if row is None:
            row = self.Q[key] = np.zeros(4)
        return row

    def choose(self, key, classes, rng, greedy=False):
        """The class to play: ε-greedy over the legal classes (random ties)."""
        raise NotImplementedError  # TODO

    def train(self, envs, steps, seed=0):
        """Q-learning for `steps` batch steps. Returns (win rate over finished fights, number of
        fights)."""
        raise NotImplementedError  # TODO

    def act(self, obs, legal, rng):
        """Greedy actions for a batch (evaluation)."""
        out = np.zeros(len(obs), dtype=int)
        for i in range(len(obs)):
            cls = self.choose(real_key(obs[i], legal[i]), legal_classes(legal[i]), rng, greedy=True)
            out[i] = class_action(cls, legal[i])
        return out


def evaluate(act, n_envs=64, n_steps=1500, seed=7):
    """Win rate of `act(obs, legal, rng) -> actions`, and the number of fights."""
    envs = make_envs(n_envs, seed=seed)
    rng = np.random.default_rng(seed)
    wins = episodes = 0
    for _ in range(n_steps):
        envs.step(act(envs.obs, envs.legal(), rng))
        episodes += int(envs.done.sum())
        wins += int(envs.won.sum())
    envs.close()
    return wins / max(1, episodes), episodes


if __name__ == "__main__":
    import time
    V_mc, counts = mc_values(heuristic_policy, coarse_key, 5000)
    V_td = td0_values(heuristic_policy, coarse_key, 5000)
    print(f"V(start) heuristic: MC {start_value(V_mc, heuristic_policy, coarse_key):+.3f}  "
          f"TD(0) {start_value(V_td, heuristic_policy, coarse_key):+.3f}  (win rate 0.80 → +0.60)")
    print("n-step targets of r=[0,0,0,1], V=0.5 everywhere:",
          [nstep_targets([0, 0, 0, 1], [0.5] * 5, [0, 0, 0, 1], n, 0.9).round(3).tolist() for n in (1, 2, 4)])
    t0 = time.time()
    agent = RealTableAgent()
    envs = make_envs(64, seed=0)
    for k in range(4):
        wr, n = agent.train(envs, 1000)
        print(f"after {(k + 1) * 1000} steps: rows {len(agent.Q)}  training win rate {wr:.3f} over {n} fights")
    envs.close()
    print(f"greedy win rate {evaluate(agent.act)[0]:.3f}  ({time.time() - t0:.0f}s)")
