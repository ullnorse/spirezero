"""RLN27: `Sts2RunEnvs.copy` is an exact mid-fight state copy: a copied fight played with the same
actions as the original has the same observation, mask and end at every step.

    python -m unittest python/tests/test_rln27_copy.py   (after the ffi build)
"""

import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

import numpy as np  # noqa: E402

import e2e_eval as EV  # noqa: E402
import rln19_act2 as R  # noqa: E402
import rln27_lookahead as L  # noqa: E402
from sts2 import HOLD  # noqa: E402

BENCH = pathlib.Path(__file__).resolve().parents[1] / "rln19/act2-boss-bench-v2.jsonl.gz"


class CopyTest(unittest.TestCase):
    def test_copy_plays_identically(self):
        _, entries = R.read_jsonl(BENCH)
        entries = [next(e for e in entries if e["boss"] == b) for b in R.BOSSES]
        per = 3  # slot 0 original, 1 and 2 copies
        env = L.replay(entries, per)
        rng = np.random.default_rng(7)
        main = np.arange(len(entries)) * per
        live = np.ones(len(entries), dtype=bool)
        steps = 0
        for _ in range(400):
            if not live.any():
                break
            # copy the original into both other slots (a fresh copy every few steps)
            if steps % 5 == 0:
                for k in (1, 2):
                    env.copy(main[live], main[live] + k)
                for k in (1, 2):
                    for f in np.nonzero(live)[0]:
                        self.assertTrue(np.array_equal(env.obs[main[f] + k], env.obs[main[f]]))
                        self.assertTrue(np.array_equal(env.masks[main[f] + k], env.masks[main[f]]))
            acts = np.full(env.n, HOLD, dtype=np.uint16)
            for f in np.nonzero(live)[0]:
                bits = np.unpackbits(env.masks[main[f]].view(np.uint8), bitorder="little")
                a = int(rng.choice(np.nonzero(bits)[0]))
                for k in range(per):
                    acts[main[f] + k] = a
            env.step(acts)
            for f in np.nonzero(live)[0]:
                for k in (1, 2):
                    i = main[f] + k
                    self.assertTrue(np.array_equal(env.obs[i], env.obs[main[f]]), (f, k, steps))
                    self.assertTrue(np.array_equal(env.masks[i], env.masks[main[f]]))
                    self.assertEqual(env.events[i, 1], env.events[main[f], 1])
                    self.assertEqual(env.done[i], env.done[main[f]])
                fin, _, _ = L.outcome(env, main[f])
                if fin:
                    live[f] = False
            steps += 1
        env.close()
        self.assertGreater(steps, 10)


if __name__ == "__main__":
    unittest.main()
