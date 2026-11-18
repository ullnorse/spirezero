"""RLN14: the batch KL probe (`kl_batch`, `kl_source`) and the A9-est of `e2e_eval.py`.

    python -m unittest python/tests/test_e2e_rln14.py   (after the ffi build)
"""

import contextlib
import io
import pathlib
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

import torch  # noqa: E402

import e2e_eval as V  # noqa: E402
import train_e2e as T  # noqa: E402


def tiny_config(**kw):
    base = dict(envs=8, threads=1, steps=8, minibatch=32, micro=16, d=32, layers=1,
                probe_rows=16, warmup_iters=2, device="cpu", deterministic=True,
                seed_range=(300_000_000, 400_000_000), ckpt_every=10**12)
    base.update(kw)
    return T.Config.from_dict({**T.dataclasses.asdict(T.Config()), **base})


class BatchKl(unittest.TestCase):
    def setUp(self):
        self.dir = pathlib.Path(tempfile.mkdtemp())

    def tearDown(self):
        shutil.rmtree(self.dir, ignore_errors=True)

    def first(self, **kw):
        learner = T.Learner(tiny_config(**kw), self.dir / "r")
        rec = learner.iterate()
        learner.close()
        shutil.rmtree(self.dir / "r", ignore_errors=True)
        return rec

    def test_kl_batch_is_logged_beside_kl(self):
        rec = self.first()
        self.assertEqual(len(rec["kls_batch"]), len(rec["kls"]))
        self.assertGreater(rec["kl_batch"], 0)
        self.assertNotEqual(rec["kl_batch"], rec["kl"])  # a different set of rows
        self.assertEqual(rec["kl_batch"], rec["kls_batch"][-1])

    def test_the_probe_is_a_stratified_draw_of_the_current_batch(self):
        learner = T.Learner(tiny_config(steps=16, probe_rows=24), self.dir / "p")
        buf, _ = learner.collect()
        flat = {k: v.flatten(0, 1) for k, v in buf.items() if torch.is_tensor(v)}
        rows, widths = learner._batch_probe(flat)
        self.assertEqual(len(rows), 24)  # 128 rows, 24 strata of one: 24 rows
        self.assertEqual(len(set(rows.tolist())), 24)
        again, _ = learner._batch_probe(flat)
        self.assertTrue((rows == again).all())  # a draw of (seed, iteration)
        learner.iteration += 1
        other, _ = learner._batch_probe(flat)
        self.assertFalse((rows == other).all())
        learner.close()

    def test_the_source_picks_what_drives_the_lr_factor(self):
        # The fixed KL is under kl_low and the batch KL between the thresholds.
        kw = dict(kl_low=1e-7, kl_high=1e-3)
        fixed = self.first(**kw)
        batch = self.first(kl_source="batch", **kw)
        self.assertLess(fixed["kl"], 1e-7)
        self.assertGreater(fixed["kl_batch"], 1e-7)
        self.assertEqual(fixed["lr_factor"], 1.5)
        self.assertEqual(batch["lr_factor"], 1.0)

    def test_the_source_drives_the_early_stop(self):
        # A threshold between the two KLs after epoch 1 stops the source that is above it only.
        base = self.first()
        k_fixed, k_batch = base["kls"][0], base["kls_batch"][0]
        self.assertNotEqual(k_fixed, k_batch)
        stop = (k_fixed + k_batch) / 2
        epochs = {src: self.first(kl_source=src, kl_stop=stop)["epochs"]
                  for src in ("fixed", "batch")}
        above = "batch" if k_batch > k_fixed else "fixed"
        below = "fixed" if above == "batch" else "batch"
        self.assertEqual((epochs[above], epochs[below]), (1, 2))

    def test_kl_source_is_settable_at_a_resume(self):
        self.assertIn("kl_source", T.RESUME_SETTABLE)
        cfg = tiny_config()
        self.assertEqual(cfg.kl_source, "fixed")
        T.set_resumed(cfg, "kl_source", "batch")
        self.assertEqual(cfg.kl_source, "batch")
        with self.assertRaises(ValueError):
            T.set_resumed(cfg, "kl_source", "nope")


class A9Est(unittest.TestCase):
    @staticmethod
    def run_row(win, boss_acts, floor=10):
        return dict(win=win, boss_acts=boss_acts, floor=floor, end="won" if win else "died",
                    nan=0, bosses=bin(boss_acts).count("1"), kf=[])

    def test_a_win_or_an_act_3_boss_kill_counts(self):
        self.assertTrue(V.a9_est(self.run_row(True, 0b111)))
        self.assertTrue(V.a9_est(self.run_row(False, 0b111)))  # beat the first act-3 boss
        self.assertFalse(V.a9_est(self.run_row(False, 0b011)))
        self.assertFalse(V.a9_est(self.run_row(False, 0)))

    def test_summary_prints_it(self):
        runs = {1: self.run_row(True, 0b111), 2: self.run_row(False, 0b111),
                3: self.run_row(False, 0b001), 4: self.run_row(False, 0)}
        self.assertEqual(V.stats_of(runs)["a9"], 2)
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            V.print_summary(dict(kind="net", panel="SMOKE"), runs)
        self.assertIn("A9-est 2 (50.00%)", buf.getvalue())
        self.assertAlmostEqual(V.paired(runs, runs)["a9_a"], 0.5)


if __name__ == "__main__":
    unittest.main()
