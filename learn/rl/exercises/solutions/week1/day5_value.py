"""Day 5: value by rollout on the mini fight, then the value of each real Act 1 fight.
Reference solution."""

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve()
while not (ROOT / "Cargo.toml").exists():
    ROOT = ROOT.parent
sys.path[:0] = [str(ROOT / "python"), str(ROOT / "learn/rl/exercises")]

import numpy as np  # noqa: E402

from common import make_envs, vocab  # noqa: E402
from common.minifight import ACTIONS, MiniFight, State, heuristic_policy  # noqa: E402
from solutions.week1.day2_observe import cards_first  # noqa: E402


def mc_value(state, policy, n=1000, seed=0):
    """Monte Carlo estimate of V(state) under `policy`: play the fight out `n` times from a
    copy of `state`, average the returns (+1 win, -1 loss)."""
    fight = MiniFight(seed)
    total = 0.0
    for _ in range(n):
        total += 1.0 if fight.play(policy, state.copy()).won else -1.0
    return total / n


def mc_q(state, policy, n=1000, seed=0):
    """Q(state, a) for each of the 3 actions: take `a` first, then `policy` finishes. None for
    an illegal action."""
    fight = MiniFight(seed)
    out = []
    for a in range(3):
        if not fight.legal(state)[a]:
            out.append(None)
            continue
        total = 0.0
        for _ in range(n):
            s = state.copy()
            fight.step(s, a)
            total += 1.0 if fight.play(policy, s).won else -1.0
        out.append(total / n)
    return out


def greedy_vs_policy(state, policy, n=1000, seed=0):
    """(the policy's own first action, the argmax-Q action). They differ where acting greedily
    on Q would improve the policy."""
    own = policy(state, MiniFight.legal(state), MiniFight(seed).rng)
    q = mc_q(state, policy, n, seed)
    best = max((a for a in range(3) if q[a] is not None), key=lambda a: q[a])
    return own, best


def value_by_encounter(policy, n_envs=64, n_steps=3000, seed=0):
    """V(start) of each Act 1 encounter under `policy(legal) -> actions`, by playing many
    fights and grouping their outcomes by the encounter id (u16 at bytes 4-5 of the static
    observation, read at the start of each episode). Returns {name: (value, n, standard
    error)}. The standard error of a mean of ±1 outcomes is std / sqrt(n)."""
    envs = make_envs(n_envs, seed=seed)
    names = vocab("encounters")
    current = envs.static_obs[:, 4:6].copy().view("<u2").ravel().copy()
    outcomes = {}
    for _ in range(n_steps):
        envs.step(policy(envs.legal()))
        for i in np.flatnonzero(envs.done):
            outcomes.setdefault(int(current[i]), []).append(1.0 if envs.won[i] else -1.0)
        if envs.done.any():
            current = envs.static_obs[:, 4:6].copy().view("<u2").ravel().copy()
    envs.close()
    out = {}
    for enc, g in outcomes.items():
        g = np.asarray(g)
        se = g.std(ddof=1) / np.sqrt(len(g)) if len(g) > 1 else 1.0
        out[names[enc]] = (float(g.mean()), len(g), float(se))
    return out


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
