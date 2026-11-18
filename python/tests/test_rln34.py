"""RLN34: the update's per-family entropy, the practice end-kind tally and `lr_factor_range` at a
resume.

    python -m unittest python/tests/test_rln34.py   (after the ffi build)
"""

import pathlib
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

import numpy as np  # noqa: E402
import torch  # noqa: E402

import train_e2e as T  # noqa: E402
from sts2.e2e.net import masked_stats  # noqa: E402
from sts2.env import PRACTICE_END, RUN_ENDS  # noqa: E402
from sts2.ffi import CELL_NAMES  # noqa: E402

CPU = torch.device("cpu")


def old_means(micros, n):
    """The pre-RLN34 log: each micro's family mean weighted by its row share of the minibatch."""
    acc = torch.zeros(2, dtype=torch.float64)
    for ent, c in micros:
        cf, e = c.double(), ent.double()
        frac = len(ent) / n
        acc += torch.stack([(e * cf).sum() / cf.sum().clamp(min=1),
                            (e * (1 - cf)).sum() / (1 - cf).sum().clamp(min=1)]) * frac
    return acc.tolist()


def new_means(micros):
    fam = T.FamilyEntropy(CPU)
    for ent, c in micros:
        fam.add(ent, c)
    return list(fam.means())


class Entropy(unittest.TestCase):
    def test_row_micros_give_the_old_numbers(self):
        """NECRO2-style micros (fixed row count, each with the minibatch's family share): the
        sums-and-counts mean is the old frac-weighted one."""
        g = torch.Generator().manual_seed(0)
        micros = []
        for _ in range(4):  # 4 micros of 16 rows, 6 combat each
            ent = torch.rand(16, generator=g)
            c = torch.zeros(16, dtype=torch.bool)
            c[torch.randperm(16, generator=g)[:6]] = True
            micros.append((ent, c))
        for a, b in zip(old_means(micros, 64), new_means(micros)):
            self.assertAlmostEqual(a, b, places=12)

    def test_token_budget_micros_give_one_at_a_zero_head(self):
        """NECRO3-style micros (a token budget: sorted by width, so a micro may hold no row of a
        family): a zero head's normalised entropy is 1.0 for every row, and the log says 1.0;
        the old one said less (a micro with no combat rows added 0)."""
        legal = torch.zeros(64, 8, dtype=torch.bool)
        legal[:, :3] = True
        legal[::2, 3:7] = True
        logits = torch.zeros(64, 8).masked_fill(~legal, float("-inf"))
        _, ent = masked_stats(logits)
        self.assertTrue(torch.allclose(ent, torch.ones(64)))
        combat = torch.zeros(64, dtype=torch.bool)
        combat[40:] = True  # the wide rows, at the end of the sorted minibatch
        micros = [(ent[a:b], combat[a:b]) for a, b in ((0, 30), (30, 50), (50, 64))]
        c, r = new_means(micros)
        self.assertAlmostEqual(c, 1.0, places=6)
        self.assertAlmostEqual(r, 1.0, places=6)
        oc, orun = old_means(micros, 64)
        self.assertLess(oc, 0.6)  # the first micro has no combat row
        self.assertLess(orun, 0.8)  # the last has no run row

    def test_update_logs_one_at_a_zero_head(self):
        """The learner's update with token-budget micros on a zero policy head (the fresh
        net's) logs entropy 1.0 for both families."""
        cfg = T.Config.from_dict({**T.dataclasses.asdict(T.Config()), **dict(
            envs=8, threads=1, steps=8, minibatch=32, micro=4, d=32, layers=1, probe_rows=16,
            warmup_iters=2, device="cpu", deterministic=True, obs_v2=True, obs_full=True,
            micro_width=8, seed_range=(300_000_000, 400_000_000), ckpt_every=10**12)})
        d = pathlib.Path(tempfile.mkdtemp())
        try:
            a = T.Learner(cfg, d / "z")
            buf, _ = a.collect()
            with torch.no_grad():
                for p in a.net.policy_parameters():
                    p.zero_()
            out = a.update(buf)
            a.close()
        finally:
            shutil.rmtree(d)
        self.assertAlmostEqual(out["ent_combat"], 1.0, places=4)
        self.assertAlmostEqual(out["ent_run"], 1.0, places=4)


class PracticeEnds(unittest.TestCase):
    def test_end_kinds_kills_and_survival(self):
        b3 = CELL_NAMES.index("BOSS_ACT3") + 1
        e2 = CELL_NAMES.index("ELITE_ACT2") + 1
        died, won = RUN_ENDS.index("died"), RUN_ENDS.index("victory")
        cap = RUN_ENDS.index("fight_capped")
        ended = np.array([b3, b3, b3, b3, e2, e2, 0])
        end = np.array([died, died, won, cap, PRACTICE_END, died, died])
        won_ = (end == won).astype(np.uint8)
        bosses = np.array([0, 1, 2, 1, 0, 0, 0])
        done = np.ones(7, bool)
        out = {}
        T.practice_ends(out, done, ended, end, won_, bosses)
        self.assertEqual(out["BOSS_ACT3"], [4, 3])  # the first boss killed in 3 of 4
        self.assertEqual(out["ELITE_ACT2"], [2, 1])
        self.assertEqual(out["ends"]["BOSS_ACT3"], {"died": 2, "victory": 1, "fight_capped": 1})
        self.assertEqual(out["ends"]["ELITE_ACT2"], {"practice_end": 1, "died": 1})
        self.assertEqual(out["kills"], {"BOSS_ACT3": [1, 2, 1]})


class LrRange(unittest.TestCase):
    def test_settable_at_a_resume_and_clamped(self):
        self.assertIn("lr_factor_range", T.RESUME_SETTABLE)
        cfg = T.Config.from_dict(T.dataclasses.asdict(T.Config()))
        T.set_resumed(cfg, "lr_factor_range", [0.3, 1.0])
        self.assertEqual(cfg.lr_factor_range, (0.3, 1.0))
        back = T.Config.from_dict(T.dataclasses.asdict(cfg))
        self.assertEqual(back.lr_factor_range, (0.3, 1.0))
        for bad in ([1.0, 0.5], [0, 1]):
            with self.assertRaises(ValueError):
                T.set_resumed(cfg, "lr_factor_range", bad)

    def test_a_resume_clamps_the_saved_factor(self):
        cfg = T.Config.from_dict({**T.dataclasses.asdict(T.Config()), **dict(
            envs=8, threads=1, steps=8, minibatch=32, micro=16, d=32, layers=1, probe_rows=16,
            warmup_iters=2, device="cpu", deterministic=True,
            seed_range=(300_000_000, 400_000_000), ckpt_every=10**12)})
        d = pathlib.Path(tempfile.mkdtemp())
        try:
            a = T.Learner(cfg, d / "r")
            a.iterate()
            a.lr_factor = 2.0
            a.save()
            a.close()
            saved = T.Config.from_dict(torch.load(d / "r" / "state.pt",
                                                  weights_only=False)["config"])
            self.assertEqual(saved.lr_factor_range, (0.3, 2.0))
            T.set_resumed(saved, "lr_factor_range", [0.3, 1.0])
            b = T.Learner(saved, d / "r", resume=True)
            self.assertEqual(b.lr_factor, 1.0)
            b.close()
        finally:
            shutil.rmtree(d)


if __name__ == "__main__":
    unittest.main()
