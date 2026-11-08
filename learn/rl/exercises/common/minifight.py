"""The mini fight from the browser toys, in Python: 28 HP, 3 energy, 5 Strikes (6 damage) and 5
Defends (5 block), five cards a turn, against a 48 HP Worm with Chomp (12), Thrash (8 + 6 block)
and Bellow (+4 Strength, 8 block). Chomp and Bellow never repeat back to back. Reward: +1 on a
win, -1 on a loss, 0 otherwise. Same rules as `learn/rl/fight.js`, so numbers from the toys and
from your code agree within sampling noise.

    fight = MiniFight(seed=1)
    s = fight.reset()
    while not s.done:
        a = heuristic(s, fight.legal(s), fight.rng)
        reward, done = fight.step(s, a)
"""

import random
from dataclasses import dataclass, replace

STRIKE, DEFEND, END = 0, 1, 2
ACTIONS = ("Strike", "Defend", "End turn")

PLAYER_HP = 28
ENEMY_HP = 48
ENERGY = 3
HAND_SIZE = 5
STRIKES = 5
DEFENDS = 5
STRIKE_DMG = 6
DEFEND_BLOCK = 5
MOVES = {
    "chomp": dict(name="Chomp", dmg=12, block=0, str=0),
    "thrash": dict(name="Thrash", dmg=8, block=6, str=0),
    "bellow": dict(name="Bellow", dmg=0, block=8, str=4),
}
WEIGHTS = dict(bellow=0.45, thrash=0.30, chomp=0.25)
MAX_STEPS = 400


@dataclass
class State:
    player_hp: int = PLAYER_HP
    block: int = 0
    energy: int = ENERGY
    enemy_hp: int = ENEMY_HP
    enemy_block: int = 0
    strength: int = 0
    intent: str = "chomp"
    last_intent: str = None
    turn: int = 1
    strikes: int = 0  # in hand
    defends: int = 0
    draw_s: int = STRIKES
    draw_d: int = DEFENDS
    disc_s: int = 0
    disc_d: int = 0
    done: bool = False
    won: bool = False
    steps: int = 0

    def copy(self):
        return replace(self)

    def incoming(self):
        """Damage the enemy would do this turn, before block."""
        m = MOVES[self.intent]
        return m["dmg"] + self.strength if m["dmg"] else 0


class MiniFight:
    def __init__(self, seed=0):
        self.rng = random.Random(seed)

    def _draw(self, s):
        s.strikes = s.defends = 0
        for _ in range(HAND_SIZE):
            if s.draw_s + s.draw_d == 0:
                s.draw_s, s.draw_d, s.disc_s, s.disc_d = s.disc_s, s.disc_d, 0, 0
            if s.draw_s + s.draw_d == 0:
                break
            if self.rng.random() * (s.draw_s + s.draw_d) < s.draw_s:
                s.draw_s -= 1
                s.strikes += 1
            else:
                s.draw_d -= 1
                s.defends += 1

    def reset(self):
        s = State()
        self._draw(s)
        return s

    @staticmethod
    def legal(s):
        """(Strike, Defend, End turn) legality."""
        if s.done:
            return (False, False, False)
        return (s.strikes > 0 and s.energy > 0, s.defends > 0 and s.energy > 0, True)

    def _pick_intent(self, s):
        for _ in range(10):
            r = self.rng.random()
            k = "bellow" if r < WEIGHTS["bellow"] else "thrash" if r < WEIGHTS["bellow"] + WEIGHTS["thrash"] else "chomp"
            if k != "thrash" and k == s.last_intent:
                continue
            return k
        return "thrash"

    def step(self, s, a):
        """Applies `a` to `s` in place; returns (reward, done)."""
        if s.done:
            return 0, True
        if not self.legal(s)[a]:
            raise ValueError(f"illegal action {ACTIONS[a]}")
        s.steps += 1
        if a == STRIKE:
            s.strikes -= 1
            s.energy -= 1
            s.disc_s += 1
            absorbed = min(s.enemy_block, STRIKE_DMG)
            s.enemy_block -= absorbed
            s.enemy_hp = max(0, s.enemy_hp - (STRIKE_DMG - absorbed))
            if s.enemy_hp == 0:
                s.done = s.won = True
                return 1, True
            return 0, False
        if a == DEFEND:
            s.defends -= 1
            s.energy -= 1
            s.disc_d += 1
            s.block += DEFEND_BLOCK
            return 0, False
        # End turn: discard the hand, the enemy acts, next turn.
        s.disc_s += s.strikes
        s.disc_d += s.defends
        s.strikes = s.defends = 0
        m = MOVES[s.intent]
        s.enemy_block = 0
        if m["dmg"]:
            raw = m["dmg"] + s.strength
            absorbed = min(s.block, raw)
            s.block -= absorbed
            s.player_hp = max(0, s.player_hp - (raw - absorbed))
        if m["block"]:
            s.enemy_block = m["block"]
        if m["str"]:
            s.strength += m["str"]
        if s.player_hp == 0:
            s.done = True
            s.won = False
            return -1, True
        s.last_intent, s.intent = s.intent, self._pick_intent(s)
        s.turn += 1
        s.block = 0
        s.energy = ENERGY
        self._draw(s)
        return 0, False

    def play(self, policy, s=None):
        """Plays a whole fight (from `s`, in place, or a fresh one) with
        `policy(state, legal, rng) -> action`. Returns the final state."""
        if s is None:
            s = self.reset()
        while not s.done and s.steps < MAX_STEPS:
            reward, done = self.step(s, policy(s, self.legal(s), self.rng))
        return s


# ---- fixed policies -----------------------------------------------------------------------
def random_policy(s, legal, rng):
    return rng.choice([a for a in range(3) if legal[a]])


def random_card_policy(s, legal, rng):
    """Random over cards; ends the turn only when nothing is playable."""
    cards = [a for a in (STRIKE, DEFEND) if legal[a]]
    return rng.choice(cards) if cards else END


def berserk_policy(s, legal, rng):
    return STRIKE if legal[STRIKE] else END


def turtle_policy(s, legal, rng):
    if legal[DEFEND] and s.incoming() > s.block:
        return DEFEND
    return STRIKE if legal[STRIKE] else END


def heuristic_policy(s, legal, rng):
    """Block if the hit would otherwise hurt, else attack."""
    inc = max(0, s.incoming() - s.block)
    if legal[STRIKE] and s.enemy_hp - s.enemy_block <= STRIKE_DMG:
        return STRIKE
    if legal[DEFEND] and inc > 0 and s.player_hp <= inc + 6:
        return DEFEND
    if legal[STRIKE]:
        return STRIKE
    if legal[DEFEND] and inc > 0:
        return DEFEND
    return END


POLICIES = dict(random=random_policy, random_card=random_card_policy, berserk=berserk_policy,
                turtle=turtle_policy, heuristic=heuristic_policy)


def win_rate(policy, n, seed=0):
    fight = MiniFight(seed)
    return sum(fight.play(policy).won for _ in range(n)) / n


if __name__ == "__main__":
    for name, pol in POLICIES.items():
        print(f"{name:12s} {win_rate(pol, 5000):.3f}")
