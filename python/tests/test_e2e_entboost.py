"""RLN35: the entropy collapse rule (`ent_boost`).

    python -m unittest python/tests/test_e2e_entboost.py   (after the ffi build)
"""

import pathlib
import shutil
import sys
import tempfile
import unittest
from types import SimpleNamespace

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import torch  # noqa: E402

import train_e2e as T  # noqa: E402
from test_e2e_rln14 import tiny_config  # noqa: E402


def rule(**kw):
    return SimpleNamespace(**{**dict(ent_boost_floor=(0.10, 0.05), ent_boost_iters=3,
                                     ent_boost_max=2), **kw})


class Logic(unittest.TestCase):
    def run_rule(self, cfg, seq):
        mult, streak, boosts = [1.0, 1.0], [0, 0], [0, 0]
        hist = []
        for ents in seq:
            T.ent_boost_step(cfg, ents, mult, streak, boosts)
            hist.append((list(mult), list(streak), list(boosts)))
        return hist

    def test_streak_boost_reset_and_cap(self):
        h = self.run_rule(rule(), [(0.05, 0.2)] * 9)
        self.assertEqual([x[0][0] for x in h], [1, 1, 1.5, 1.5, 1.5, 2.25, 2.25, 2.25, 2.25])
        self.assertEqual(h[2][1][0], 0)  # streak reset after a boost
        self.assertEqual(h[-1][2], [2, 0])  # capped at 2; run never under its floor
        self.assertEqual(h[-1][0][1], 1.0)

    def test_a_recovery_breaks_the_streak(self):
        h = self.run_rule(rule(), [(0.05, 0.2), (0.05, 0.2), (0.12, 0.2), (0.05, 0.2), (0.05, 0.2)])
        self.assertEqual(h[-1][0][0], 1.0)
        self.assertEqual(h[-1][1][0], 2)

    def test_types_are_independent(self):
        h = self.run_rule(rule(), [(0.5, 0.01)] * 3)
        self.assertEqual(h[-1][0], [1.0, 1.5])


class Learner(unittest.TestCase):
    def setUp(self):
        self.dir = pathlib.Path(tempfile.mkdtemp())

    def tearDown(self):
        shutil.rmtree(self.dir, ignore_errors=True)

    def run_n(self, name, n, **kw):
        a = T.Learner(tiny_config(**kw), self.dir / name)
        recs = [a.iterate() for _ in range(n)]
        params = [p.detach().clone() for p in a.net.parameters()]
        a.close()
        return recs, params

    def test_off_is_identical_and_inert_rule_too(self):
        _, base = self.run_n("a", 3)
        _, off = self.run_n("b", 3, ent_boost=False, ent_boost_floor=(1.0, 1.0), ent_boost_iters=1)
        _, inert = self.run_n("c", 3, ent_boost=True, ent_boost_floor=(0.0, 0.0))
        for x, y, z in zip(base, off, inert):
            self.assertTrue(torch.equal(x, y))
            self.assertTrue(torch.equal(x, z))

    def test_on_boosts_and_changes_the_update(self):
        recs, params = self.run_n("d", 4, ent_boost=True, ent_boost_floor=(1.0, 1.0),
                                  ent_boost_iters=1)
        self.assertEqual([r["ent_mult"] for r in recs],
                         [[1.5, 1.5], [2.25, 2.25], [2.25, 2.25], [2.25, 2.25]])  # after the update
        _, base = self.run_n("e", 4)
        self.assertFalse(all(torch.equal(x, y) for x, y in zip(base, params)))

    def test_save_resume_round_trip_and_old_checkpoint(self):
        cfg = tiny_config(ent_boost=True, ent_boost_floor=(1.0, 1.0), ent_boost_iters=2)
        a = T.Learner(cfg, self.dir / "f")
        a.iterate()
        a.iterate()
        a.save(self.dir / "f" / "state.pt")
        ref = [a.iterate() for _ in range(2)]
        a.close()
        (self.dir / "g").mkdir()
        shutil.copy(self.dir / "f" / "state.pt", self.dir / "g" / "state.pt")
        b = T.Learner(cfg, self.dir / "g", resume=True)
        self.assertEqual((b.ent_mult, b.ent_boosts), ([1.5, 1.5], [1, 1]))
        got = [b.iterate() for _ in range(2)]
        b.close()
        self.assertEqual([r["ent_mult"] for r in got], [r["ent_mult"] for r in ref])
        self.assertEqual([r["ent_coef"] for r in got], [r["ent_coef"] for r in ref])
        # a checkpoint without the key (older) resumes at multiplier 1
        s = torch.load(self.dir / "g" / "state.pt", weights_only=False)
        del s["ent_boost"]
        (self.dir / "h").mkdir()
        torch.save(s, self.dir / "h" / "state.pt")
        c = T.Learner(cfg, self.dir / "h", resume=True)
        self.assertEqual(c.ent_mult, [1.0, 1.0])
        c.close()

    def test_settable_at_resume(self):
        for k in ("ent_boost", "ent_boost_floor", "ent_boost_iters", "ent_boost_max"):
            self.assertIn(k, T.RESUME_SETTABLE)
        cfg = tiny_config()
        T.set_resumed(cfg, "ent_boost_floor", [0.2, 0.1])
        self.assertEqual(cfg.ent_boost_floor, (0.2, 0.1))


if __name__ == "__main__":
    unittest.main()
