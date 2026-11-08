"""The A10 whole-run baseline (`docs/specs/a10-training.md`).

Plays a fixed, held-out seed set on every character at ascension 10 in one of two modes:

- `random`: the agent plays every decision, fights included, with a uniform random legal
  index per step (`sts2.policy.RandomLegal`);
- `greedy`: the agent makes the run-level choices with the same random policy and the env's
  greedy resolver (`CombatMode::Greedy`) plays every fight.

`python python/a10_baseline.py [--seeds N] [--envs N] [--modes random,greedy]` prints one row
per (character, mode) and writes the same numbers as JSON and CSV under `target/a10-baseline/`;
nothing there is committed. `--seeds` counts runs per character (default 200), drawn from
`TEST_SEED_BASE` in order. The trainer's held-out contract is in the spec and in the constants
below; the harness must not be pointed at the training range.

A step is one agent decision per env; automatic single-option decisions taken inside the env are
counted in `actions`/`auto`, not in `steps`. `steps/s` is agent decisions per second over the
whole call (env construction and every reset included), `resets/s` is finished episodes per
second, and the two rejection counts are:

- `rej`: policy choices that were not set in the current mask (the harness substitutes the
  first legal index and counts each; a mask policy should have zero);
- `refused`: the env's own `refused` counter: listed actions dropped from the mask because
  `Run::apply` would refuse them, plus resolver positions where every action was refused.

Only numpy and `python/sts2/` are used. The library must be built first:
`cargo build --profile ffi -p sts2-ffi`.
"""

import argparse
import csv
import json
import pathlib
import statistics
import sys
import time

import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

from sts2 import ffi  # noqa: E402
from sts2.env import CHARACTERS, RUN_ENDS, Sts2RunEnvs  # noqa: E402
from sts2.policy import RandomLegal  # noqa: E402

ASCENSION = 10

# The seed sets of the training contract (docs/specs/a10-training.md). TEST is what this
# harness plays; the same seeds are played by every character.
TRAIN_SEED_BASE = 1_000_000
TRAIN_SEEDS = 100_000
DEV_SEED_BASE = 8_000_000
DEV_SEEDS = 1_000
TEST_SEED_BASE = 9_000_000
TEST_SEEDS = 200

# `--mode` names to `Sts2RunEnvs(combat=...)`; "random" is a random agent in the fights too.
MODES = {"random": "agent", "greedy": "greedy", "random-resolver": "random"}
DEFAULT_MODES = ("random", "greedy")


class Result:
    """The aggregate of one (character, mode) run over its seeds."""

    def __init__(self, character, mode):
        self.character = character
        self.mode = mode
        self.episodes = 0
        self.wins = 0
        self.floors = []
        self.ends = [0] * len(RUN_ENDS)
        self.decisions = 0
        self.actions = 0
        self.auto = 0
        self.resolved = 0
        self.refused = 0
        self.unmapped = 0
        self.mask_rejections = 0
        self.seconds = 0.0
        self.step_calls = 0

    @property
    def win_rate(self):
        return self.wins / self.episodes if self.episodes else 0.0

    @property
    def median_floor(self):
        return statistics.median(self.floors) if self.floors else 0.0

    @property
    def mean_floor(self):
        return statistics.fmean(self.floors) if self.floors else 0.0

    @property
    def decisions_per_episode(self):
        return self.decisions / self.episodes if self.episodes else 0.0

    @property
    def steps_per_s(self):
        return self.decisions / self.seconds if self.seconds else 0.0

    @property
    def actions_per_s(self):
        return self.actions / self.seconds if self.seconds else 0.0

    @property
    def resets_per_s(self):
        return self.episodes / self.seconds if self.seconds else 0.0

    def row(self):
        return {
            "character": self.character,
            "mode": self.mode,
            "episodes": self.episodes,
            "wins": self.wins,
            "win_rate": self.win_rate,
            "median_floor": self.median_floor,
            "mean_floor": self.mean_floor,
            "decisions": self.decisions,
            "decisions_per_episode": self.decisions_per_episode,
            "actions": self.actions,
            "auto": self.auto,
            "resolved": self.resolved,
            "steps_per_s": self.steps_per_s,
            "actions_per_s": self.actions_per_s,
            "resets_per_s": self.resets_per_s,
            "mask_rejections": self.mask_rejections,
            "refused": self.refused,
            "unmapped": self.unmapped,
            "step_calls": self.step_calls,
            "seconds": self.seconds,
            "ends": {
                name: self.ends[i]
                for i, name in enumerate(RUN_ENDS)
                if i and self.ends[i]
            },
        }


def _first_legal(masks):
    """The lowest set bit of each mask (0 for an empty mask)."""
    bits = np.unpackbits(masks.view(np.uint8), axis=1, bitorder="little")
    return bits.argmax(axis=1)


def evaluate(
    character, mode, seeds, envs_n=16, threads=1, lib=None, policy_base=TEST_SEED_BASE
):
    """Plays `seeds` runs of `character` in `mode` at ascension 10 on `envs_n` envs."""
    combat = MODES[mode]
    char_index = CHARACTERS.index(character)
    # A fixed policy stream per (character, mode): reruns play the same draws.
    policy_seed = policy_base + char_index * 7919 + list(MODES).index(mode) * 104729
    policy = RandomLegal(envs_n, policy_seed)
    result = Result(character, mode)
    start = time.perf_counter()
    envs = Sts2RunEnvs(
        envs_n,
        seeds=list(seeds),
        character=character,
        ascension=ASCENSION,
        combat=combat,
        threads=threads,
        lib=lib,
    )
    rows = np.arange(envs_n)
    idle = np.zeros(envs_n, dtype=np.uint16)
    try:
        while result.episodes < len(seeds):
            masks = envs.masks
            live = masks.any(axis=1)
            if live.any():
                actions = policy(masks).astype(np.uint16)
                chosen = ((masks[rows, actions >> 6] >> (actions & 63)) & 1).astype(bool)
                rejected = live & ~chosen
                if rejected.any():
                    result.mask_rejections += int(rejected.sum())
                    actions = np.where(rejected, _first_legal(masks), actions).astype(np.uint16)
            else:
                # An episode that ended at its reset is reported on the next step; step once
                # more to drain it, and stop only if no episode was reported.
                actions = idle
            envs.step(actions)
            stats = envs.stats()
            result.step_calls += 1
            result.decisions += stats["decisions"]
            result.actions += stats["actions"]
            result.auto += stats["auto"]
            result.resolved += stats["resolved"]
            result.refused += stats["refused"]
            result.unmapped += stats["unmapped"]
            done = envs.done.astype(bool)
            if done.any():
                result.episodes += int(done.sum())
                result.wins += int(envs.won[done].sum())
                result.floors.extend(int(f) for f in envs.floor[done])
                for end in envs.end[done]:
                    result.ends[int(end)] += 1
            elif not live.any():
                break
    finally:
        envs.close()
    result.seconds = time.perf_counter() - start
    if result.episodes != len(seeds):
        raise RuntimeError(
            f"{character}/{mode}: {result.episodes} of {len(seeds)} seeded episodes ended"
        )
    return result


def total(results, mode):
    """The sum of `results` (steps/s and resets/s weighted by their own seconds)."""
    out = Result("TOTAL", mode)
    for r in results:
        out.episodes += r.episodes
        out.wins += r.wins
        out.floors.extend(r.floors)
        out.ends = [a + b for a, b in zip(out.ends, r.ends)]
        out.decisions += r.decisions
        out.actions += r.actions
        out.auto += r.auto
        out.resolved += r.resolved
        out.refused += r.refused
        out.unmapped += r.unmapped
        out.mask_rejections += r.mask_rejections
        out.seconds += r.seconds
        out.step_calls += r.step_calls
    return out


def print_table(results, seed_base, seeds_per_character, envs_n, threads):
    print(
        f"A10 baseline: seeds {seed_base}..{seed_base + seeds_per_character - 1} "
        f"({seeds_per_character} per character), envs={envs_n} threads={threads}"
    )
    header = (
        f"{'mode':<8} {'character':<11} {'eps':>5} {'win%':>6} "
        f"{'floor med/mean':>15} {'dec/ep':>7} {'steps/s':>9} {'acts/s':>9} "
        f"{'reset/s':>8} {'rej':>5} {'refused':>8}"
    )
    print(header)
    print("-" * len(header))
    for r in results:
        print(
            f"{r.mode:<8} {r.character:<11} {r.episodes:>5} {100 * r.win_rate:>6.1f} "
            f"{r.median_floor:>7.1f}/{r.mean_floor:<7.1f} {r.decisions_per_episode:>7.1f} "
            f"{r.steps_per_s:>9,.0f} {r.actions_per_s:>9,.0f} {r.resets_per_s:>8.1f} "
            f"{r.mask_rejections:>5} {r.refused:>8}"
        )
    print()


def write_results(results, out_dir, metadata):
    out_dir = pathlib.Path(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    payload = dict(metadata)
    payload["rows"] = [r.row() for r in results]
    (out_dir / "a10-baseline.json").write_text(json.dumps(payload, indent=2) + "\n")
    columns = list(results[0].row()) if results else []
    with (out_dir / "a10-baseline.csv").open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=columns)
        writer.writeheader()
        for r in results:
            row = r.row()
            row["ends"] = json.dumps(row["ends"], separators=(",", ":"))
            writer.writerow(row)
    return out_dir / "a10-baseline.json", out_dir / "a10-baseline.csv"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--seeds", type=int, default=TEST_SEEDS, help="runs per character")
    parser.add_argument("--seed-base", type=int, default=TEST_SEED_BASE)
    parser.add_argument("--envs", type=int, default=16, help="envs in the batch")
    parser.add_argument("--threads", type=int, default=1)
    parser.add_argument("--modes", default=",".join(DEFAULT_MODES))
    parser.add_argument(
        "--characters", default=",".join(CHARACTERS), help="comma-separated names"
    )
    parser.add_argument("--out", default="target/a10-baseline", help="output directory")
    args = parser.parse_args(argv)
    modes = [m for m in args.modes.split(",") if m]
    characters = [c for c in args.characters.split(",") if c]
    unknown = [c for c in characters if c not in CHARACTERS]
    if unknown:
        parser.error(f"unknown character(s): {', '.join(unknown)}")
    unknown = [m for m in modes if m not in MODES]
    if unknown:
        parser.error(f"unknown mode(s): {', '.join(unknown)}")
    if not modes or not characters:
        parser.error("--modes and --characters must not be empty")
    if args.seeds < 1 or args.envs < 1:
        parser.error("--seeds and --envs must be positive")
    if args.seeds < args.envs:
        parser.error("--seeds must be at least --envs (a seed list is assigned in env order)")

    lib = ffi.load()
    seeds = range(args.seed_base, args.seed_base + args.seeds)
    results = []
    for mode in modes:
        for character in characters:
            result = evaluate(
                character, mode, seeds, envs_n=args.envs, threads=args.threads, lib=lib,
                policy_base=args.seed_base,
            )
            results.append(result)
        results.append(total(results[-len(characters):], mode))

    print_table(results, args.seed_base, args.seeds, args.envs, args.threads)
    for mode in modes:
        row = next(r for r in results if r.mode == mode and r.character == "TOTAL")
        ends = ", ".join(
            f"{name}={count}" for name, count in zip(RUN_ENDS, row.ends) if count
        )
        print(
            f"{mode}: {row.episodes} episodes, {row.wins} wins, {row.decisions:,} decisions, "
            f"{row.actions:,} engine actions ({row.auto:,} automatic, {row.resolved:,} resolved), "
            f"{row.seconds:.2f}s, ends: {ends}"
        )
    metadata = {
        "ascension": ASCENSION,
        "seed_base": args.seed_base,
        "seeds_per_character": args.seeds,
        "envs": args.envs,
        "threads": args.threads,
        "modes": modes,
        "characters": characters,
        "abi_version": int(lib.sts2_abi_version()),
        "build_id": lib.sts2_build_id().decode(),
    }
    paths = write_results(results, args.out, metadata)
    print(f"wrote {paths[0]} and {paths[1]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
