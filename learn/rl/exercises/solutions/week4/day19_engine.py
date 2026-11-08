"""Day 19: the engine side. What the mask and the buffers promise, checked from Python over
thousands of steps; what an illegal action does; steps per second against threads to size a
batch. Reference solution.

Run:  .venv/bin/python learn/rl/exercises/week4/day19_engine.py
"""

import pathlib
import sys
import time

ROOT = pathlib.Path(__file__).resolve()
while not (ROOT / "Cargo.toml").exists():
    ROOT = ROOT.parent
sys.path[:0] = [str(ROOT / "python"), str(ROOT / "learn/rl/exercises")]

import numpy as np  # noqa: E402

from common import ACTIONS, DONE, END_TURN, HAND_SLOTS, PICK_BASE, POTION_BASE, POTION_SLOTS, TARGET_SLOTS, make_envs  # noqa: E402

MAX_EPISODE_ACTIONS = 10_000


def check_header(obs):
    """Violations of the header's promises over a batch (n × 972 uint8), as a list of
    strings, empty if none: 0 ≤ hp ≤ max_hp, block ≥ 0, energy ≥ 0, max_energy ≥ 0 (i16 at
    0, 2, 4, 6, 8), hand ≤ 10 (byte 16), status in 0..4 (byte 23)."""
    obs = np.asarray(obs, dtype=np.uint8)
    i16 = obs[:, :10].copy().view("<i2")
    out = []
    for i in range(len(obs)):
        hp, max_hp, block, energy, max_energy = (int(v) for v in i16[i])
        if not 0 <= hp <= max_hp:
            out.append(f"env {i}: hp {hp} of {max_hp}")
        if block < 0 or energy < 0 or max_energy < 0:
            out.append(f"env {i}: block {block} energy {energy}/{max_energy}")
        if obs[i, 16] > HAND_SLOTS:
            out.append(f"env {i}: {obs[i, 16]} cards in hand")
        if obs[i, 23] > 4:
            out.append(f"env {i}: status {obs[i, 23]}")
    return out


def check_mask(obs, legal):
    """Violations of the mask's promises over a batch, as strings: every env has at least one
    legal action (the env resets a finished fight before you see it); a legal play index
    names a hand slot below the hand size (byte 16); a legal potion index names a belt slot
    below 6; at most one of End turn and Done is legal; no bit at or past ACTIONS (the mask
    has exactly ACTIONS columns)."""
    obs = np.asarray(obs, dtype=np.uint8)
    out = []
    if legal.shape[1] != ACTIONS:
        out.append(f"mask has {legal.shape[1]} columns, not {ACTIONS}")
        return out
    for i in range(len(obs)):
        row = legal[i]
        if not row.any():
            out.append(f"env {i}: empty mask")
        plays = np.flatnonzero(row[:POTION_BASE]) // TARGET_SLOTS
        if plays.size and plays.max() >= obs[i, 16]:
            out.append(f"env {i}: play from slot {plays.max()} with {obs[i, 16]} cards in hand")
        potions = np.flatnonzero(row[POTION_BASE:END_TURN]) // TARGET_SLOTS
        if potions.size and potions.max() >= POTION_SLOTS:
            out.append(f"env {i}: potion slot {potions.max()}")
        if row[END_TURN] and row[DONE]:
            out.append(f"env {i}: End turn and Done both legal")
    return out


def check_stats(stats, n, done, won):
    """Violations of `envs.stats()` after a step of `n` envs, as strings: actions == n, ended
    == done.sum(), won == won.sum() and ≤ ended, capped ≤ ended, reset == ended (the default
    starts never run out)."""
    out = []
    done_n, won_n = int(np.sum(done)), int(np.sum(won))
    if stats["actions"] != n:
        out.append(f"actions {stats['actions']} for {n} envs")
    if stats["ended"] != done_n:
        out.append(f"ended {stats['ended']} but done {done_n}")
    if stats["won"] != won_n or stats["won"] > stats["ended"]:
        out.append(f"won {stats['won']} but won flags {won_n} of {stats['ended']}")
    if stats["capped"] > stats["ended"]:
        out.append(f"capped {stats['capped']} of {stats['ended']}")
    if stats["reset"] != stats["ended"]:
        out.append(f"reset {stats['reset']} of {stats['ended']} ended")
    return out


def illegal_is_rejected(envs):
    """Steps `envs` with an illegal index in env 0 (the lowest index whose bit is clear) and
    legal ones elsewhere. Returns (error message or None, observation unchanged: bool)."""
    import sts2

    legal = envs.legal()
    actions = legal.argmax(axis=1)
    actions[0] = int(np.flatnonzero(~legal[0])[0])
    before = envs.obs.copy()
    try:
        envs.step(actions)
    except sts2.Sts2Error as e:
        return str(e), bool(np.array_equal(before, envs.obs))
    return None, bool(np.array_equal(before, envs.obs))


def audit(policy, n_envs=64, n_steps=2000, seed=0, threads=1):
    """Runs `policy(legal) -> actions` for `n_steps` steps and checks the header, the mask and
    the stats after every step. Returns (violations: list of (step, message), fights ended,
    fights capped)."""
    envs = make_envs(n_envs, seed=seed, threads=threads)
    violations, ended, capped = [], 0, 0
    for t in range(n_steps):
        legal = envs.legal()
        for m in check_header(envs.obs) + check_mask(envs.obs, legal):
            violations.append((t, m))
        envs.step(policy(legal))
        stats = envs.stats()
        for m in check_stats(stats, n_envs, envs.done, envs.won):
            violations.append((t, m))
        ended += stats["ended"]
        capped += stats["capped"]
    envs.close()
    return violations, ended, capped


def bench(threads_list=(1, 2, 4, 8), n_envs=256, n_steps=500, seed=0):
    """Env steps per second of random legal play for each thread count, as {threads:
    steps_per_second}, with the same actions for every count (a fresh batch and policy per
    count)."""
    from sts2.policy import RandomLegal

    out = {}
    for threads in threads_list:
        envs = make_envs(n_envs, seed=seed, threads=threads)
        policy = RandomLegal(n_envs, seed)
        t0 = time.perf_counter()
        for _ in range(n_steps):
            envs.step(policy(envs.masks))
        out[threads] = n_envs * n_steps / (time.perf_counter() - t0)
        envs.close()
    return out


if __name__ == "__main__":
    from solutions.week1.day2_observe import cards_first

    violations, ended, capped = audit(cards_first, n_steps=2000)
    print(f"audit: {len(violations)} violations over 2,000 steps × 64 envs, {ended} fights, {capped} capped")
    for t, m in violations[:5]:
        print(f"  step {t}: {m}")
    envs = make_envs(4, seed=1)
    msg, unchanged = illegal_is_rejected(envs)
    envs.close()
    print(f"illegal action: {msg!r}, observation unchanged: {unchanged}")
    for threads, sps in bench().items():
        print(f"threads {threads}: {sps:,.0f} steps/s")
