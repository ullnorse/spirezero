"""RLN40: the upgrade bonus (`reward.upgrade`) and `deck_upgrades`.

    PYTHONPATH=<venv site-packages> python -m pytest -q python/tests -k rln40   (after the ffi build)
"""

import dataclasses
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
from test_e2e_rln14 import tiny_config  # noqa: E402


def row(entries):
    o = np.zeros((1, 400), dtype=np.uint8)
    for k, (card, up) in enumerate(entries):
        o[0, T.DECK_AT + 4 * k:T.DECK_AT + 4 * k + 4] = [card & 255, card >> 8, up, 0]
    return o


class DeckUpgrades(unittest.TestCase):
    def test_counts_levels_of_present_cards(self):
        self.assertEqual(deck(row([(5, 0), (6, 1), (300, 2)])), 3)
        self.assertEqual(deck(row([])), 0)

    def test_stray_upgrade_bytes_of_empty_slots_do_not_count(self):
        self.assertEqual(deck(row([(0, 0), (0, 7), (9, 1)])), 1)


def deck(o):
    return int(T.deck_upgrades(o)[0])


class RealEnv(unittest.TestCase):
    def test_smith_raises_the_count_in_a_real_obs(self):
        from sts2 import E2E, Sts2RunEnvs
        env = Sts2RunEnvs(16, seeds=np.arange(16, dtype=np.uint64) + 7, character="Necrobinder",
                          ascension=10, combat="greedy", threads=2, **E2E)
        smith, best = 236 + 1, 0  # REST_BASE + the Smith slot (Heal is slot 0)
        try:
            for _ in range(1500):
                acts = np.zeros(16, dtype=np.uint16)
                for i in range(16):
                    idx = np.nonzero(np.unpackbits(env.masks[i].view(np.uint8), bitorder="little"))[0]
                    acts[i] = smith if smith in idx else (idx[0] if len(idx) else 0)
                env.step(acts)
                best = max(best, int(T.deck_upgrades(env.obs).max()))
        finally:
            env.close()
        self.assertGreaterEqual(best, 2)


class Flag(unittest.TestCase):
    def test_default_off_and_resume_merge(self):
        self.assertEqual(T.Reward().upgrade, 0.0)
        cfg = tiny_config()
        T.set_resumed(cfg, "reward", {"upgrade": 0.001})
        self.assertEqual(dataclasses.asdict(cfg.reward),
                         dict(win=1.0, boss=0.05, floor=0.002, upgrade=0.001))
        self.assertIn("reward", T.RESUME_SETTABLE)
        # an older checkpoint's reward dict (no `upgrade`) loads as off
        self.assertEqual(T.Config.from_dict({**dataclasses.asdict(tiny_config()),
                                             "reward": dict(win=1.0, boss=0.05, floor=0.002)}
                                            ).reward.upgrade, 0.0)


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

    def test_zero_is_identical_and_on_runs(self):
        r0, base = self.run_n("a", 2)
        r1, zero = self.run_n("b", 2, reward=dict(upgrade=0.0))
        self.assertTrue(all(torch.equal(x, y) for x, y in zip(base, zero)))
        r2, on = self.run_n("c", 2, reward=dict(upgrade=0.5))
        self.assertTrue(all(np.isfinite(r["pg"]) for r in r2))
        self.assertTrue(all(torch.isfinite(x).all() for x in on))  # the path runs (no upgrade in 2 tiny iterations)
