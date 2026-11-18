"""RLN28: `Sts2RunEnvs.copy(..., reseed=...)` hides the hidden future and nothing visible: a
reseeded copy has the source's observation and mask, the same seed plays the same, and other
seeds draw other futures.

    python -m unittest python/tests/test_rln28_reseed.py   (after the ffi build)
"""

import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

import numpy as np  # noqa: E402

import rln19_act2 as R  # noqa: E402
import rln27_lookahead as L  # noqa: E402
from sts2 import HOLD  # noqa: E402

BENCH = pathlib.Path(__file__).resolve().parents[1] / "rln19/act2-boss-bench-v2.jsonl.gz"


class ReseedTest(unittest.TestCase):
    def setUp(self):
        _, entries = R.read_jsonl(BENCH)
        self.entries = [next(e for e in entries if e["boss"] == b) for b in R.BOSSES]

    def test_reseed_keeps_the_visible_and_is_deterministic(self):
        per = 5  # 0 original; 1, 2 seed 11; 3 seed 12; 4 seed 13
        env = L.replay(self.entries, per)
        main = np.arange(len(self.entries)) * per
        src = np.repeat(main, 4)
        dst = src + np.tile([1, 2, 3, 4], len(main))
        seeds = np.tile(np.asarray([11, 11, 12, 13], dtype=np.uint64), len(main))
        env.copy(src, dst, reseed=seeds)
        for f in main:
            for k in range(1, per):
                self.assertTrue(np.array_equal(env.obs[f + k], env.obs[f]), (f, k))
                self.assertTrue(np.array_equal(env.masks[f + k], env.masks[f]), (f, k))
        rng = np.random.default_rng(3)
        live = np.ones(len(main), dtype=bool)
        diverged = np.zeros(len(main), dtype=bool)
        for _ in range(300):
            if not live.any():
                break
            acts = np.full(env.n, HOLD, dtype=np.uint16)
            for f in np.nonzero(live)[0]:
                # one random legal action of copy 1, for every slot that can take it
                bits = np.unpackbits(env.masks[main[f] + 1].view(np.uint8), bitorder="little")
                a = int(rng.choice(np.nonzero(bits)[0]))
                for k in range(per):
                    i = main[f] + k
                    legal = np.unpackbits(env.masks[i].view(np.uint8), bitorder="little")
                    acts[i] = a if legal[a] and not L.outcome(env, i)[0] else HOLD
            env.step(acts)
            for f in np.nonzero(live)[0]:
                a, b = main[f] + 1, main[f] + 2
                self.assertTrue(np.array_equal(env.obs[a], env.obs[b]), f)  # same seed
                self.assertTrue(np.array_equal(env.masks[a], env.masks[b]), f)
                if any(not np.array_equal(env.obs[main[f] + k], env.obs[a]) for k in (0, 3, 4)):
                    diverged[f] = True
                if L.outcome(env, a)[0]:
                    live[f] = False
        env.close()
        # Other seeds (and the real streams) draw a different future. Not in every fight: random
        # play can lose a fight (Knowledge Demon here) before a draw or roll shows anything.
        self.assertGreaterEqual(int(diverged.sum()), 2, diverged)

    def test_reseed_needs_one_seed_per_copy(self):
        env = L.replay(self.entries[:1], 2)
        with self.assertRaises(ValueError):
            env.copy([0], [1], reseed=[1, 2])
        env.close()


if __name__ == "__main__":
    unittest.main()
