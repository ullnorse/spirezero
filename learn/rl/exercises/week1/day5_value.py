"""Day 5: value by rollout on the mini fight, then the value of each real Act 1 fight.

Run:  .venv/bin/python learn/rl/exercises/week1/day5_value.py
Test: .venv/bin/python -m pytest learn/rl/exercises/week1/test_day5.py
"""

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve()
while not (ROOT / "Cargo.toml").exists():
    ROOT = ROOT.parent
sys.path[:0] = [str(ROOT / "python"), str(ROOT / "learn/rl/exercises")]

import numpy as np  # noqa: E402

from common import make_envs, vocab  # noqa: E402
from common.minifight import ACTIONS, MiniFight, State, heuristic_policy  # noqa: E402
from week1.day2_observe import cards_first  # noqa: E402  (your day 2)


def mc_value(state, policy, n=1000, seed=0):
    """Monte Carlo estimate of V(state) under `policy`: play the fight out `n` times from a
    copy of `state` (`state.copy()`; `MiniFight(seed).play(policy, s)` plays `s` to the end in
    place), average the returns (+1 win, -1 loss)."""
    raise NotImplementedError  # TODO


def mc_q(state, policy, n=1000, seed=0):
    """Q(state, a) for each of the 3 actions: take `a` first (`fight.step(s, a)`), then
    `policy` finishes. None for an illegal action (`MiniFight.legal(state)`)."""
    raise NotImplementedError  # TODO


def greedy_vs_policy(state, policy, n=1000, seed=0):
    """(the policy's own first action, the argmax-Q action). They differ where acting greedily
    on Q would improve the policy."""
    raise NotImplementedError  # TODO


def value_by_encounter(policy, n_envs=64, n_steps=3000, seed=0):
    """V(start) of each Act 1 encounter under `policy(legal) -> actions`, by playing many
    fights and grouping their outcomes by encounter. Returns {name: (value, n, standard
    error)}; names from `vocab("encounters")`.

    The encounter id is the u16 at bytes 4-5 of `envs.static_obs`, which is rewritten when an
    env resets. Read it at the start of every episode (after construction and again after any
    step where `envs.done` is set) and attribute each outcome to the id the episode started
    with. The standard error of a mean of ±1 outcomes is std(ddof=1) / sqrt(n)."""
    raise NotImplementedError  # TODO


if __name__ == "__main__":
    # HP 10, Worm 12 with Strength 8, Chomp (20) incoming, 3 Strikes 2 Defends in hand.
    s = State(player_hp=10, enemy_hp=12, strength=8, intent="chomp", strikes=3, defends=2,
              draw_s=2, draw_d=3, turn=2)
    print(f"V(s) heuristic {mc_value(s, heuristic_policy, 2000):+.3f}")
    q = mc_q(s, heuristic_policy, 2000)
    print("Q(s, a):", {ACTIONS[a]: (None if v is None else round(v, 3)) for a, v in enumerate(q)})
    own, best = greedy_vs_policy(s, heuristic_policy, 2000)
    print(f"policy plays {ACTIONS[own]}, greedy on Q plays {ACTIONS[best]}")
    table = value_by_encounter(cards_first)
    for name, (v, n, se) in sorted(table.items(), key=lambda kv: kv[1][0]):
        print(f"{name:34s} V {v:+.2f} ± {se:.2f}  ({n} fights)")
