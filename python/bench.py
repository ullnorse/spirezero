"""Steps per second of the binding from Python: `python python/bench.py [ENVS] [STEPS] [THREADS]
[SEED]` (defaults 64, 20,000, 1, 0). The workload and output are those of the Rust driver
`crates/sts2-ffi/examples/ffi_bench.rs`, so the two lines compare directly (same actions,
episodes, wins and observation digest; only the time differs).
"""

import pathlib
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

from sts2 import Sts2Envs  # noqa: E402
from sts2.policy import RandomLegal  # noqa: E402


def fnv(data):
    h = 0xCBF29CE484222325
    for b in data:
        h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def main(argv):
    args = [int(a) for a in argv[1:]]
    n, steps, threads, seed = (args + [64, 20_000, 1, 0][len(args):])[:4]
    envs = Sts2Envs(n, seed=seed, threads=threads)
    policy = RandomLegal(n, seed)
    applied = episodes = wins = 0
    start = time.perf_counter()
    for _ in range(steps):
        episodes += envs.step(policy(envs.masks))
        applied += envs.stats()["actions"]
        wins += int(envs.won.sum())
    seconds = time.perf_counter() - start
    digest = fnv(envs.obs.tobytes())
    print(
        f"python envs={n} threads={threads} steps={steps} actions={applied} "
        f"episodes={episodes} wins={wins} obs_digest={digest:016x} seconds={seconds:.3f} "
        f"steps_per_s={applied / seconds:.0f}"
    )
    envs.close()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
