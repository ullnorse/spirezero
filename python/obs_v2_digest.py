"""RLN17: a digest of the run observation bytes (v2 section included) over SMOKE runs.

`python obs_v2_digest.py [--decisions N] [--envs E] [--threads T]`: A10 Necrobinder runs on
the SMOKE seeds (23,000,000+, `e2e_eval.py`) with the trainer's env options and `obs_v2`, each
decision a uniform legal action drawn with a fixed numpy RNG. Hashes (SHA-256) every step's
obs rows and masks until N decisions are taken, and prints a JSON line with the digest, the
decisions and the rows whose v2 section is not zero. A change that keeps the obs bytes keeps
the digest; it does not depend on `--threads`.

Recorded at RLN16's merge (47b2d01), before RLN17's speed work:
- default args (102,400 decisions): 18dd8fc15dee9a2d8854619f10666d6a3d49c7331a316096f2d1e8dd30296624
- `--decisions 1024000 --envs 512`: f3e3b7e01c71636cfa9c198c52efccdef222e66cc2ea882deed8c43bd5f44200

ABI 9 (RLN21) appended bytes to every row, so the whole-row digest changed; `--width 3392`
hashes each row's ABI-8 part. Default args at 44e73e9 (ABI 8, 100,096 decisions):
727f9a1e0813db0815c69bc6d953bf349304810b08e32665c87e109902e03189, the same as RLN21's
`--width 3392`; RLN21's whole rows: 7bc99c9a4ec7e4ae1e240647571662f10f9d9c33de4c43f85c6fc313f9accf19.

ABI 10 (RLN22) appended the plan column: `--width 3696` (the ABI-9 rows) gives RLN21's whole-row
digest above; RLN22's whole rows: a9c9c8936342fab74e35bf901bdb3682b12964e13d75b9b4149a59aeeb0faf7e.
"""

import argparse
import hashlib
import json
import pathlib
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import numpy as np  # noqa: E402

from sts2.e2e import encode as E  # noqa: E402
from sts2.env import E2E, Sts2RunEnvs  # noqa: E402

SMOKE_FIRST = 23_000_000


def uniform_actions(masks, rng):
    """A uniform legal index per row of `masks` (n x words uint64); 0 for an empty mask."""
    bits = np.unpackbits(masks.view(np.uint8), axis=1, bitorder="little").astype(bool)
    u = rng.random(bits.shape)
    return np.where(bits, u, -1.0).argmax(1).astype(np.uint16)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--decisions", type=int, default=100_000)
    ap.add_argument("--envs", type=int, default=256)
    ap.add_argument("--threads", type=int, default=2)
    ap.add_argument("--width", type=int, default=0,
                    help="hash only each row's first WIDTH bytes (3392: ABI-8 rows, 3696: ABI-9)")
    args = ap.parse_args(argv)
    envs = Sts2RunEnvs(args.envs, seed_range=(SMOKE_FIRST, SMOKE_FIRST + 1_000_000),
                       character="Necrobinder", ascension=10, combat="agent",
                       threads=args.threads, priv=True, obs_v2=True, **E2E)
    E.check_layout(envs.layout, v2=True)
    rng = np.random.default_rng(17)
    h = hashlib.sha256()
    decisions = v2_rows = steps = 0
    t0 = time.perf_counter()
    while decisions < args.decisions:
        h.update((envs.obs[:, :args.width] if args.width else envs.obs).tobytes())
        h.update(envs.masks.tobytes())
        v2_rows += int((envs.obs[:, E.V2_AT:] != 0).any(1).sum())
        envs.step(uniform_actions(envs.masks, rng))
        decisions += args.envs
        steps += 1
    print(json.dumps(dict(digest=h.hexdigest(), decisions=decisions, steps=steps,
                          rows_with_v2_bytes=v2_rows,
                          secs=round(time.perf_counter() - t0, 1))))


if __name__ == "__main__":
    main()
