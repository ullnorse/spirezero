"""The run env's end-to-end options through the binding (`docs/plans/necro-e2e.md`, WP1):
`python -m unittest discover -s python/tests` (after `cargo build --profile ffi -p sts2-ffi`).
With `**E2E` the mask holds one index per action class and `class_counts` their sizes, no
decision with one class reaches the agent, the events add up to the final floor, and the action
table names an entity for every index."""

import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

import numpy as np  # noqa: E402

from sts2 import ffi  # noqa: E402
from sts2.env import (  # noqa: E402
    E2E, ENTITY_KINDS, EVENT_FIELDS, RUN_ENDS, Sts2RunEnvs, action_table,
)

ENVS = 8


class RunE2E(unittest.TestCase):
    def test_classes_events_and_counters(self):
        env = Sts2RunEnvs(ENVS, seed_range=(23_000_000, 23_000_200), ascension=10, threads=2,
                          **E2E)
        rng = np.random.default_rng(0)
        floor_at_start = env.obs[:, 7].astype(np.int64)
        floors = np.zeros(ENVS, dtype=np.int64)
        runs = merged = 0
        saturated = dict.fromkeys(ffi.SATURATIONS, 0)
        for _ in range(20_000):
            legal = env.legal()
            live = legal.any(axis=1)
            if not live.any():
                break
            self.assertTrue((legal[live].sum(axis=1) >= 2).all(), "one class reaches no agent")
            self.assertTrue(((env.class_counts > 0) == legal).all())
            scores = np.where(legal, rng.random(legal.shape), -1.0)
            ended = env.step(scores.argmax(axis=1))
            stats = env.stats()
            merged += stats["merged"]
            for k, v in stats["saturated"].items():
                saturated[k] += v
            floors += env.events[:, EVENT_FIELDS.index("floors")]
            if ended:
                done = env.done.astype(bool)
                np.testing.assert_array_equal((floor_at_start + floors)[done], env.floor[done])
                ends = env.end[done]
                self.assertTrue(np.isin(ends, [RUN_ENDS.index(e) for e in (
                    "died", "victory", "fight_capped", "decision_capped")]).all(), ends)
                runs += ended
                floors[done] = 0
                floor_at_start[done] = env.obs[done, 7]
        self.assertEqual(runs, 200)
        self.assertGreater(merged, 0)
        self.assertLessEqual(saturated["any"], sum(saturated.values()))
        env.close()

    def test_action_table(self):
        table = action_table()
        lib = ffi.load()
        layout = ffi.run_layout(lib)
        self.assertEqual(table.shape, (int(layout.actions), 4))
        self.assertTrue((table[:, 0] < len(ENTITY_KINDS)).all())
        # Index 0 plays hand slot 0 untargeted in a fight; proceed is the 355th, then the (RLN32) discard slots.
        self.assertEqual(table[0].tolist(), [0, 0, 0, 3])
        self.assertEqual(ENTITY_KINDS[table[354, 0]], "proceed")
        self.assertEqual(ENTITY_KINDS[table[-1, 0]], "discard_potion")
        self.assertEqual(len({tuple(r) for r in table.tolist()}), len(table))

    def test_options_off_write_nothing(self):
        env = Sts2RunEnvs(4, seed_range=(0, 4))
        rng = np.random.default_rng(1)
        for _ in range(100):
            legal = env.legal()
            if not legal.any():
                break
            env.step(np.where(legal, rng.random(legal.shape), -1.0).argmax(axis=1))
            self.assertFalse(env.class_counts.any())
            self.assertFalse(env.events.any())
        env.close()


class RunPriv(unittest.TestCase):
    def test_priv_view_is_zero_off_and_filled_on_without_changing_the_obs(self):
        kw = dict(seed_range=(23_000_000, 23_000_100), ascension=10, **E2E)
        off, on = Sts2RunEnvs(4, **kw), Sts2RunEnvs(4, priv=True, **kw)
        self.assertEqual(on.priv.shape, (4, on.priv_bytes))
        seen = False
        for _ in range(300):
            self.assertTrue((off.priv == 0).all())
            self.assertTrue((off.obs == on.obs).all())
            seen |= bool(on.priv.any())
            legal = off.legal()
            if not legal.any():
                break
            actions = legal.argmax(axis=1)
            off.step(actions)
            on.step(actions)
        self.assertTrue(seen)


if __name__ == "__main__":
    unittest.main()
