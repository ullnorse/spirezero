"""Two-chain returns, run-row hold-back and per-type advantage moments (RLN15,
`sts2/e2e/returns.py`): `python -m unittest discover -s python/tests`.

Hand-built sequences: combat -> run -> combat, a run that ends inside the rollout, a held run
row carried into the next iteration (its advantage equals the one computed over both
iterations at once), and a truncated practice segment. Each is also compared with
spire-mentor's own `compute_returns` and `AdvNorm` when that checkout is present (imported
read-only, no bytecode written). The learner trains every stored row exactly once with
`returns="two_chain"`, and resumes with `adv_norm="per_type"`.
"""

import importlib
import pathlib
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import numpy as np  # noqa: E402

from sts2.e2e import returns as R  # noqa: E402

MENTOR = pathlib.Path("~/projects/spire-mentor/sts2-pycli/python").expanduser()


def theirs():
    """spire-mentor's `ppo.e2e.algo`, or None."""
    if not (MENTOR / "ppo" / "e2e" / "algo.py").exists():
        return None
    old = sys.dont_write_bytecode
    sys.dont_write_bytecode = True
    sys.path.insert(0, str(MENTOR))
    try:
        return importlib.import_module("ppo.e2e.algo")
    except Exception:  # noqa: BLE001 - their deps may be missing here
        return None
    finally:
        sys.path.remove(str(MENTOR))
        sys.dont_write_bytecode = old


def table(rows):
    """rows: (run_uid, t, combat, reward, value, done, bootstrap)."""
    cols = list(zip(*rows))
    return dict(run_uid=np.array(cols[0], np.int64), t=np.array(cols[1], np.int64),
                is_combat=np.array(cols[2], bool), reward=np.array(cols[3], np.float64),
                value=np.array(cols[4], np.float64), done=np.array(cols[5], bool),
                bootstrap=np.array(cols[6], bool), dtype=np.zeros(len(rows), np.int64),
                ref=np.arange(len(rows), dtype=np.int64))


LC, LR = 0.95, 0.97


class TwoChain(unittest.TestCase):
    def check_theirs(self, tab, ret):
        algo = theirs()
        if algo is None:
            self.skipTest("spire-mentor not importable")
        t = {k: v for k, v in tab.items() if k not in ("value", "ref", "dtype")}
        t["actor"] = np.zeros(len(t["run_uid"]), np.int64)
        got = algo.compute_returns(t, tab["value"], lam_c=LC, lam_r=LR, hold_iters=4)
        np.testing.assert_allclose(ret["adv"], got.adv, rtol=0, atol=1e-12, equal_nan=True)
        np.testing.assert_allclose(ret["target"], got.target, rtol=0, atol=1e-12,
                                   equal_nan=True)
        np.testing.assert_array_equal(ret["trained"], got.trained)
        np.testing.assert_array_equal(ret["held"], got.held)

    def test_combat_run_combat_with_run_end(self):
        # run row 0, fight (1, 2), run row 3, fight (4, 5) ending the run (terminal).
        v = [0.5, 0.4, 0.45, 0.6, 0.3, 0.2]
        r = [0.0, 0.0, 0.002, 0.0, 0.0, 0.05]
        rows = [(7, i, c, r[i], v[i], i == 5, False)
                for i, c in enumerate([False, True, True, False, True, True])]
        tab = table(rows)
        ret = R.compute_returns(tab, lam_c=LC, lam_r=LR)
        a = ret["adv"]
        # Combat chain stops at the fight's end, bootstrapping from the next run row's value.
        d2 = r[2] + v[3] - v[2]
        d1 = r[1] + v[2] - v[1]
        self.assertAlmostEqual(a[2], d2)
        self.assertAlmostEqual(a[1], d1 + LC * d2)
        d5 = r[5] + 0.0 - v[5]
        self.assertAlmostEqual(a[5], d5)
        self.assertAlmostEqual(a[4], r[4] + v[5] - v[4] + LC * d5)
        # Run chain: R sums every reward up to the next run row; the run ended, so it trains.
        d3 = r[3] + r[4] + r[5] + 0.0 - v[3]
        d0 = r[0] + r[1] + r[2] + v[3] - v[0]
        self.assertAlmostEqual(a[3], d3)
        self.assertAlmostEqual(a[0], d0 + LR * d3)
        self.assertTrue(ret["trained"].all())
        self.assertFalse(ret["held"].any())
        self.check_theirs(tab, ret)

    def test_open_run_is_cut_and_its_run_rows_held(self):
        # Two envs' runs in one table: run 1 ends; run 2 is still open (bootstrap row).
        rows = [(1, 0, False, 0.0, 0.3, False, False), (1, 1, True, 0.0, 0.2, False, False),
                (1, 2, True, 1.0, 0.1, True, False),
                (2, 0, False, 0.002, 0.5, False, False), (2, 1, True, 0.0, 0.6, False, False),
                (2, 2, True, 0.0, 0.55, False, False), (2, 3, False, 0.0, 0.7, False, True)]
        tab = table(rows)
        ret = R.compute_returns(tab, lam_c=LC, lam_r=LR)
        np.testing.assert_array_equal(ret["trained"], [1, 1, 1, 0, 1, 1, 0])
        np.testing.assert_array_equal(ret["held"], [0, 0, 0, 1, 0, 0, 0])
        # The cut fight bootstraps from the bootstrap row's value.
        d5 = 0.7 - 0.55
        self.assertAlmostEqual(ret["adv"][5], d5)
        self.assertAlmostEqual(ret["adv"][4], 0.55 - 0.6 + LC * d5)
        self.assertAlmostEqual(ret["boot_w"][4], LC)
        self.check_theirs(tab, ret)

    def test_held_row_carried_over(self):
        """A held run row trains later with the advantage it has over both iterations."""
        v = [0.5, 0.4, 0.45, 0.6, 0.3, 0.2, 0.55, 0.25]
        r = [0.002, 0.0, 0.05, 0.002, 0.0, 0.0, 0.002, 1.0]
        comb = [False, True, True, False, True, True, False, True]
        whole = table([(3, i, comb[i], r[i], v[i], i == 7, False) for i in range(8)])
        ref = R.compute_returns(whole, lam_c=LC, lam_r=LR)
        # Iteration 1: rows 0-2, cut at row 3 (bootstrap); with hold_iters 4 row 0 waits.
        it1 = table([(3, i, comb[i], r[i], v[i], False, False) for i in range(3)]
                    + [(3, 3, False, 0.0, v[3], False, True)])
        tab = R.join({}, it1)
        ret = R.compute_returns(tab, lam_c=LC, lam_r=LR)
        np.testing.assert_array_equal(ret["held"], [1, 0, 0, 0])
        np.testing.assert_array_equal(ret["trained"], [0, 1, 1, 0])
        held = R.carry(tab, ret)
        # The fight's rewards left with its rows and are folded into the held row.
        self.assertEqual(held["reward"].tolist(), [r[0] + r[1] + r[2]])
        self.assertEqual(held["age"].tolist(), [1])
        # Iteration 2: rows 3-7, the run ends: the held row trains.
        it2 = table([(3, i, comb[i], r[i], v[i], i == 7, False) for i in range(3, 8)])
        tab = R.join(held, it2)
        ret = R.compute_returns(tab, lam_c=LC, lam_r=LR)
        self.assertTrue(ret["trained"].all())
        np.testing.assert_allclose(ret["adv"][0], ref["adv"][0], rtol=0, atol=1e-12)
        np.testing.assert_allclose(ret["adv"][1:], ref["adv"][3:], rtol=0, atol=1e-12)
        self.check_theirs(tab, ret)

    def test_release_at_hold_iters(self):
        it = table([(5, 0, False, 0.0, 0.5, False, False), (5, 1, False, 0.0, 0.4, False, True)])
        held = {}
        for k in range(5):
            tab = R.join(held, {kk: vv.copy() for kk, vv in it.items()})
            tab["t"] = np.where(tab["carried"], tab["t"], tab["t"] + 2 * k)
            ret = R.compute_returns(tab, lam_c=LC, lam_r=LR, hold_iters=2)
            held = R.carry(tab, ret)
            if k >= 2:  # the oldest row reached age 2 and trained
                self.assertTrue(ret["trained"][0])
            self.assertLessEqual(len(held["run_uid"]), 2)

    def test_practice_segment_bootstraps(self):
        # A map move (run row) into an elite, the fight, truncated at the first decision
        # outside combat: a done bootstrap row with that position's value.
        rows = [(9, 0, False, 0.002, 0.3, False, False), (9, 2, True, 0.0, 0.35, False, False),
                (9, 4, True, 0.0, 0.4, False, False), (9, 5, False, 0.0, 0.42, True, True)]
        tab = table(rows)
        ret = R.compute_returns(tab, lam_c=LC, lam_r=LR)
        np.testing.assert_array_equal(ret["trained"], [1, 1, 1, 0])
        self.assertFalse(ret["held"].any())  # the segment is over: its run row trains now
        d2 = 0.42 - 0.4
        self.assertAlmostEqual(ret["adv"][2], d2)
        self.assertAlmostEqual(ret["adv"][0], 0.002 + 0.42 - 0.3)
        self.assertAlmostEqual(ret["boot_w"][0], 1.0)
        self.check_theirs(tab, ret)


class AdvMoments(unittest.TestCase):
    def test_matches_theirs(self):
        algo = theirs()
        if algo is None:
            self.skipTest("spire-mentor not importable")
        rng = np.random.default_rng(3)
        ours, ref = R.AdvNorm(64), algo.AdvNorm(64)
        for _ in range(6):
            n = 300
            d = rng.choice([1, 2, 9, 12, 21, 35], n, p=[.6, .1, .15, .1, .04, .01])
            a = rng.normal(size=n) * (1 + d / 10)
            comb = d <= 2
            got = ours.update_normalize(a, d, comb)
            want = ref.update_normalize(a, d, np.ones(n, bool), comb)
            np.testing.assert_allclose(got, want, rtol=0, atol=1e-12)
        r2 = R.AdvNorm.from_dict(ours.to_dict())
        np.testing.assert_array_equal(r2.mean, ours.mean)


class DecisionTypes(unittest.TestCase):
    def test_types_from_live_states(self):
        from sts2 import E2E, Sts2RunEnvs
        from sts2.e2e import encode as E
        env = Sts2RunEnvs(16, seed_range=(23_000_000, 23_100_000), ascension=10, threads=2,
                          priv=False, **E2E)
        rng = np.random.default_rng(0)
        seen = set()
        for _ in range(400):
            d = R.decision_types(env.obs, env.masks.view(np.uint8), E.SELECT_AT)
            combat = env.obs[:, 3] == 3
            self.assertTrue(np.all(np.isin(d[combat], [1, 2])))
            self.assertTrue(np.all(d[~combat] >= 8))
            seen.update(d.tolist())
            legal = env.legal()
            env.step(np.where(legal, rng.random(legal.shape), -1).argmax(1))
        env.close()
        self.assertTrue({1, 10}.issubset(seen))  # combat turns and map moves at least


class Learner(unittest.TestCase):
    def setUp(self):
        self.dir = pathlib.Path(tempfile.mkdtemp(dir=pathlib.Path(__file__).resolve()
                                                 .parents[2] / "target"))

    def tearDown(self):
        shutil.rmtree(self.dir, ignore_errors=True)

    def test_every_row_trains_once_and_resume_drops_held(self):
        import torch

        import train_e2e as T
        from test_e2e_learner import tiny_config
        cfg = tiny_config(returns="two_chain", adv_norm="per_type")
        a = T.Learner(cfg, self.dir)
        trained = 0
        for _ in range(4):
            rec = a.iterate()
            self.assertTrue(rec["finite"])
            trained += rec["rows_trained"]
            self.assertEqual(rec["rows_overdue"], 0)
        self.assertEqual(trained + rec["rows_held"], a.decisions)
        self.assertGreater(rec["rows_held"], 0)
        a.save()
        a.close()
        state = torch.load(self.dir / "state.pt", weights_only=False)
        self.assertIn("adv_norm", state)
        b = T.Learner(cfg, self.dir, resume=True)
        self.assertEqual(b.held, {})
        np.testing.assert_array_equal(b.advnorm.count, a.advnorm.count)
        self.assertTrue(b.iterate()["finite"])
        b.close()

    def test_practice_segment_in_a_batch(self):
        """`_finish_two_chain` on a hand-built batch: env 0 plays a practice segment (map move,
        fight) truncated at step 2 with bootstrap 0.9, then a new run; env 1's run stays open.
        The segment's rows chain to its bootstrap and train now; nothing crosses the cut."""
        import torch

        import train_e2e as T
        from test_e2e_learner import tiny_config
        cfg = tiny_config(envs=2, steps=4, returns="two_chain", adv_norm="per_type",
                          cache_tokens=False)
        a = T.Learner(cfg, self.dir)
        a.probe = {}
        s, n = 4, 2
        comb = torch.tensor([[0, 1], [1, 1], [1, 0], [0, 1]], dtype=torch.bool)
        val = torch.tensor([[0.3, 0.5], [0.35, 0.5], [0.4, 0.6], [0.2, 0.5]])
        rew = torch.tensor([[0.002, 0.0], [0.0, 0.0], [0.0, 0.002], [0.0, 0.0]])
        done = torch.zeros(s, n, dtype=torch.bool)
        done[2, 0] = True
        boot = torch.zeros(s, n)
        boot[2, 0] = 0.9
        buf = dict(masks=torch.zeros(s, n, 6, dtype=torch.int64),
                   action=torch.zeros(s, n, dtype=torch.int64), logp=torch.zeros(s, n),
                   value=val, reward=rew, done=done, combat=comb,
                   width=torch.full((s, n), 5, dtype=torch.int16),
                   dtype=torch.where(comb, 1, 10).to(torch.uint8),
                   obs=torch.zeros(s, n, 4, dtype=torch.uint8),
                   priv=torch.zeros(s, n, 4, dtype=torch.uint8),
                   last_value=torch.tensor([0.25, 0.55]), boot=boot, trunc=done.clone())
        out = a._finish_two_chain(buf)
        st = out["ret_stats"]
        self.assertEqual(st["rows_practice_cut"], 1)
        # Trained: env 0's segment (rows 0-2) and every combat row; held: the open run rows
        # (env 0's new run's map row at step 3, env 1's run row at step 2).
        self.assertEqual(st["rows_trained"], 6)
        self.assertEqual(st["rows_held"], 2)
        flat = out["flat"]
        v = flat["value"].double().numpy()
        adv = lambda x: float(flat["adv"][int(np.flatnonzero(abs(v - x) < 1e-6)[0])])  # noqa: E731
        # The fight's last row bootstraps from 0.9; the map move gets R + 0.9 - V.
        self.assertAlmostEqual(adv(0.4), 0.9 - 0.4, places=6)
        self.assertAlmostEqual(adv(0.3), 0.002 + 0.9 - 0.3, places=6)
        self.assertAlmostEqual(adv(0.35), 0.4 - 0.35 + LC * (0.9 - 0.4), places=6)
        a.close()

    def test_flags_settable_at_resume(self):
        import train_e2e as T
        self.assertIn("returns", T.RESUME_SETTABLE)
        self.assertIn("adv_norm", T.RESUME_SETTABLE)
        cfg = T.Config()
        T.set_resumed(cfg, "returns", "two_chain")
        self.assertEqual(cfg.returns, "two_chain")


if __name__ == "__main__":
    unittest.main()
