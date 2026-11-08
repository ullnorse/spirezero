"""Run-env steps and runs per second from Python: `python python/run_bench.py [ENVS] [STEPS]
[THREADS] [SEED] [COMBAT]` (defaults 64, 5,000, 1, 0, 0). The workload and output are those of
the Rust driver `crates/sts2-ffi/examples/ffi_run_bench.rs` (Necrobinder runs on seeds drawn from
master seed SEED; COMBAT 0 the agent plays the fights, 1 greedy, 2 random resolver), so the two
lines compare directly (same decisions, actions, runs, wins and observation digest; only the
time differs). A step is one agent decision per env.
"""

import pathlib
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

from sts2 import Sts2RunEnvs  # noqa: E402
from sts2.policy import RandomLegal  # noqa: E402


def fnv(data):
    h = 0xCBF29CE484222325
    for b in data:
        h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def main(argv):
    args = [int(a) for a in argv[1:]]
    n, steps, threads, seed, combat = (args + [64, 5_000, 1, 0, 0][len(args):])[:5]
    envs = Sts2RunEnvs(n, master=seed, character="Necrobinder", combat=combat, threads=threads)
    policy = RandomLegal(n, seed)
    decisions = applied = runs = wins = 0
    start = time.perf_counter()
    for _ in range(steps):
        runs += envs.step(policy(envs.masks))
        stats = envs.stats()
        decisions += stats["decisions"]
        applied += stats["actions"]
        wins += int(envs.won.sum())
    seconds = time.perf_counter() - start
    digest = fnv(envs.obs.tobytes())
    print(
        f"python-runs envs={n} threads={threads} combat={combat} steps={steps} "
        f"decisions={decisions} actions={applied} runs={runs} wins={wins} "
        f"obs_digest={digest:016x} seconds={seconds:.3f} "
        f"steps_per_s={decisions / seconds:.0f} runs_per_s={runs / seconds:.1f}"
    )
    envs.close()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
