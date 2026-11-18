"""RLN37: failure-driven practice weights (`Practice.adaptive`): the off schedule is unchanged,
the L update and weights, the death cell of an obs, and L in state.pt (and an old checkpoint).

    python -m unittest python/tests/test_e2e_rln37.py   (after the ffi build)
"""

import pathlib
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import numpy as np  # noqa: E402
import torch  # noqa: E402

import train_e2e as T  # noqa: E402
from test_e2e_practice import tiny_config  # noqa: E402


class Math(unittest.TestCase):
    def test_off_identical(self):
        p = T.Practice(enabled=True, start=0)
        base = p.schedule(5_000_000, 2048)
        self.assertEqual(p.schedule(5_000_000, 2048, [9, 0, 0, 0, 0, 1]), base)  # adaptive off
        q = T.Practice(enabled=True, start=0, adaptive=True)
        self.assertEqual(q.schedule(5_000_000, 2048, None), base)
        self.assertEqual(q.schedule(5_000_000, 2048, [0.0] * 6), base)  # no deaths yet

    def test_update(self):
        p = T.Practice(adaptive=True)
        self.assertEqual(p.adaptive_step([4, 0, 2, 0, 0, 8], [1, 2, 0, 0, 0, 1]),
                         [3, 2, 1, 0, 0, 5])

    def test_weights_floor(self):
        p = T.Practice(adaptive=True)
        w = p.adaptive_weights([0, 0, 0, 10, 0, 0])
        self.assertAlmostEqual(sum(w), 1.0)
        self.assertAlmostEqual(w[3], 0.9 + 0.1 / 6)
        self.assertAlmostEqual(w[0], 0.1 / 6)
        self.assertIsNone(p.adaptive_weights([0] * 6))
        slots, ppm = p.schedule(0, 100, [1, 1, 1, 1, 1, 1])
        self.assertEqual(slots, 40)
        self.assertTrue(all(abs(v - 1e6 / 6) <= 1 for v in ppm))

    def test_death_cell(self):
        def obs(act, room):
            o = np.zeros(64, np.uint8)
            o[2], o[26] = act, room
            return o
        self.assertEqual(T.death_cell(obs(0, 3)), 3)
        self.assertEqual(T.death_cell(obs(1, 3)), 4)
        self.assertEqual(T.death_cell(obs(2, 2)), 2)
        self.assertEqual(T.death_cell(obs(1, 2)), 1)
        self.assertIsNone(T.death_cell(obs(0, 1)))  # hallway
        self.assertIsNone(T.death_cell(obs(0, 0)))  # not in combat


class Learner(unittest.TestCase):
    def setUp(self):
        self.dir = pathlib.Path(tempfile.mkdtemp())

    def tearDown(self):
        shutil.rmtree(self.dir)

    def cfg(self, **kw):
        return tiny_config(practice=dict(enabled=True, min_active=1, max_uses=0,
                                          share=[0.5, 0.5], **kw))

    def test_window_save_resume_old(self):
        a = T.Learner(self.cfg(adaptive=True, adaptive_every=2), self.dir / "a")
        a.iteration = 1
        a._adaptive_update({"BOSS_ACT1": 3, "other": 5})
        self.assertEqual((a.adapt_loss, a.adapt_deaths[3]), ([0.0] * 6, 3))  # window open
        a.iteration = 2
        a._adaptive_update({"BOSS_ACT1": 1, "ELITE_ACT2": 2})
        self.assertEqual(a.adapt_loss, [0.0, 2.0, 0.0, 4.0, 0.0, 0.0])
        self.assertEqual(a.adapt_deaths, [0] * 6)
        a.iterate()
        a.save()
        want = (list(a.adapt_loss), list(a.adapt_deaths))
        a.close()
        st = torch.load(self.dir / "a" / "state.pt", weights_only=False)
        b = T.Learner(T.Config.from_dict(st["config"]), self.dir / "a", resume=True)
        self.assertEqual((b.adapt_loss, b.adapt_deaths), want)
        b.close()
        del st["adaptive"]  # an older checkpoint
        torch.save(st, self.dir / "a" / "state.pt")
        c = T.Learner(T.Config.from_dict(st["config"]), self.dir / "a", resume=True)
        self.assertEqual((c.adapt_loss, c.adapt_deaths), ([0.0] * 6, [0] * 6))
        self.assertTrue(c.iterate()["finite"])
        c.close()

    def test_logs_adaptive(self):
        a = T.Learner(self.cfg(adaptive=True), self.dir / "a")
        rec = a.iterate()
        ad = rec["practice"]["adaptive"]
        self.assertEqual(set(ad), {"loss", "weights", "deaths"})
        self.assertEqual(len(ad["weights"]), 6)
        a.close()
        b = T.Learner(self.cfg(), self.dir / "b")
        self.assertNotIn("adaptive", b.iterate()["practice"])
        b.close()


if __name__ == "__main__":
    unittest.main()
