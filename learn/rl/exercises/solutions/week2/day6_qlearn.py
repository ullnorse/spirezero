"""Day 6: tabular Q-learning on the mini fight. Reference solution."""

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve()
while not (ROOT / "Cargo.toml").exists():
    ROOT = ROOT.parent
sys.path[:0] = [str(ROOT / "python"), str(ROOT / "learn/rl/exercises")]

import numpy as np  # noqa: E402

from common.minifight import STRIKE_DMG, MiniFight, heuristic_policy, random_policy, win_rate  # noqa: E402


def coarse_key(s):
    """A handful of buckets: (intent, energy, strikes, defends, danger, hp bucket, strikes to
    kill). danger: 'safe' if nothing gets through the block, 'lethal' if the hit kills, 'low'
    if it leaves at most 6 HP, else 'ok'. hp bucket: 'hp1' ≤ 9, 'hp2' ≤ 18, 'hp3'. Strikes to
    kill: ceil((enemy HP + enemy block) / 6), capped at 6."""
    inc = max(0, s.incoming() - s.block)
    danger = "safe" if inc == 0 else "lethal" if s.player_hp <= inc else "low" if s.player_hp <= inc + 6 else "ok"
    hp = "hp1" if s.player_hp <= 9 else "hp2" if s.player_hp <= 18 else "hp3"
    to_kill = min(6, -(-(s.enemy_hp + s.enemy_block) // STRIKE_DMG))
    return (s.intent, s.energy, s.strikes, s.defends, danger, hp, to_kill)


def full_key(s):
    """Every number that matters: (intent, energy, strikes, defends, player HP, block, enemy
    HP, enemy block, strength)."""
    return (s.intent, s.energy, s.strikes, s.defends, s.player_hp, s.block, s.enemy_hp,
            s.enemy_block, s.strength)


def blind_key(s):
    """Ignores the enemy entirely: (energy, strikes, defends)."""
    return (s.energy, s.strikes, s.defends)


KEYS = dict(coarse=coarse_key, full=full_key, blind=blind_key)


class QAgent:
    """Tabular Q-learning. `Q[key] -> array of 3` is created at 0 on first sight."""

    def __init__(self, key=coarse_key, alpha=0.1, gamma=0.99, eps=0.1, seed=1):
        self.key, self.alpha, self.gamma, self.eps = key, alpha, gamma, eps
        self.Q = {}
        self.fight = MiniFight(seed)
        self.episodes = self.wins = self.steps = 0

    def q(self, key):
        """The row for `key`, created as zeros if new."""
        row = self.Q.get(key)
        if row is None:
            row = self.Q[key] = np.zeros(3)
        return row

    def greedy(self, s, legal):
        """The legal action with the highest Q in s's row (lowest index on ties)."""
        row = self.q(self.key(s))
        best, best_q = None, -np.inf
        for a in range(3):
            if legal[a] and row[a] > best_q:
                best, best_q = a, row[a]
        return best

    def act(self, s, legal):
        """ε-greedy: a uniform legal action with probability eps, else greedy."""
        if self.fight.rng.random() < self.eps:
            return random_policy(s, legal, self.fight.rng)
        return self.greedy(s, legal)

    def update(self, key, a, reward, done, next_key, next_legal):
        """Q(key, a) += alpha * (target - Q(key, a)), target = reward + gamma * max legal
        Q(next_key, ·), or just reward when done."""
        target = reward
        if not done:
            nxt = self.q(next_key)
            target += self.gamma * max(nxt[b] for b in range(3) if next_legal[b])
        row = self.q(key)
        row[a] += self.alpha * (target - row[a])

    def episode(self):
        """One fight: act, step, update, repeat. Returns the final state."""
        s = self.fight.reset()
        while not s.done:
            key, legal = self.key(s), self.fight.legal(s)
            a = self.act(s, legal)
            reward, done = self.fight.step(s, a)
            self.update(key, a, reward, done, self.key(s), self.fight.legal(s))
            self.steps += 1
        self.episodes += 1
        self.wins += s.won
        return s

    def policy(self, s, legal, rng):
        """The greedy policy, in the `policy(state, legal, rng)` shape the fight expects."""
        return self.greedy(s, legal)


def train(agent, episodes, window=500):
    """Runs `episodes` fights. Returns the training win rate per `window` episodes."""
    curve, wins = [], 0
    for i in range(1, episodes + 1):
        wins += agent.episode().won
        if i % window == 0:
            curve.append(wins / window)
            wins = 0
    return curve


def eps_schedule(step, start=1.0, end=0.05, over=10000):
    """Linear from `start` to `end` over `over` steps, then `end`."""
    if step >= over:
        return end
    return start + (end - start) * step / over


if __name__ == "__main__":
    for name, key in KEYS.items():
        agent = QAgent(key)
        curve = train(agent, 20000)
        print(f"{name:7s} rows {len(agent.Q):6d}  train win rate {curve[0]:.2f} → {curve[-1]:.2f}  "
              f"greedy {win_rate(agent.policy, 3000, seed=5):.3f}")
    print(f"heuristic {win_rate(heuristic_policy, 3000, seed=5):.3f}")
