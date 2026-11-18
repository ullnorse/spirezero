"""RLN47: `Sts2RunEnvs(no_full_heal=True)` drops Heal at a full-HP rest site with another option.

    python -m unittest python/tests/test_e2e_rln47.py   (after the ffi build)
"""
import unittest

import numpy as np

from sts2 import E2E, Sts2RunEnvs
from sts2.env import PHASE_REST, REST_AT, REST_BASE, REST_HEAL, REST_SLOTS


def rest_rows(env):
    """[(row, full HP, heal slot legal, other option legal)] of the envs at a rest site."""
    o, legal = env.obs, env.legal()
    out = []
    for i in np.nonzero(o[:, 3] == PHASE_REST)[0]:
        hp = int(o[i, 8]) | int(o[i, 9]) << 8
        mx = int(o[i, 10]) | int(o[i, 11]) << 8
        kind = o[i, REST_AT:REST_AT + 2 * REST_SLOTS:2]
        heal = [s for s in range(REST_SLOTS) if kind[s] == REST_HEAL + 1]
        other = [s for s in range(REST_SLOTS) if kind[s] > 0 and kind[s] != REST_HEAL + 1]
        out.append((i, hp >= mx > 0, any(legal[i, REST_BASE + s] for s in heal),
                    any(legal[i, REST_BASE + s] for s in other)))
    return out


class NoFullHeal(unittest.TestCase):
    def test(self):
        env = Sts2RunEnvs(64, seed_range=(23_000_000, 23_100_000), ascension=10, threads=2,
                          no_full_heal=True, **E2E)
        rng = np.random.default_rng(0)
        full = partial = 0
        try:
            for _ in range(3000):
                for i, at_full, heal, other in rest_rows(env):
                    if at_full and other:
                        self.assertFalse(heal, "Heal legal at full HP")
                        full += 1
                    elif not at_full and other:
                        partial += heal
                legal = env.legal()
                env.step(np.where(legal, rng.random(legal.shape), -1).argmax(1))
        finally:
            env.close()
        self.assertGreater(full, 0, "no full-HP rest site seen")
        self.assertGreater(partial, 0, "Heal must stay legal below full HP")
        self.assertGreater(env.full_heal_masked, 0)


if __name__ == "__main__":
    unittest.main()
