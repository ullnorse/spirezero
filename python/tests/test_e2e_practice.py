"""Practice starts in the env binding and the e2e learner (RLN12): the schedule, the env's
practice buffers, GAE's bootstrap, a learner with practice on (it trains, and resumes), and a
practice-off checkpoint resumed with practice turned on.

    python -m unittest python/tests/test_e2e_practice.py   (after the ffi build)
"""

import math
import pathlib
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

import numpy as np  # noqa: E402
import torch  # noqa: E402

import train_e2e as T  # noqa: E402
from sts2 import Sts2RunEnvs  # noqa: E402
from sts2.env import PRACTICE_END  # noqa: E402


def tiny_config(**kw):
    base = dict(envs=8, threads=1, steps=8, minibatch=32, micro=16, d=32, layers=1,
                probe_rows=16, warmup_iters=2, device="cpu", deterministic=True,
                seed_range=(300_000_000, 400_000_000), ckpt_every=10**12)
    base.update(kw)
    return T.Config.from_dict({**T.dataclasses.asdict(T.Config()), **base})


class Schedule(unittest.TestCase):
    def test_ramps_and_mix(self):
        p = T.Practice(enabled=True, start=1000)
        slots, w = p.schedule(1000, 2048)
        self.assertEqual(slots, round(0.40 * 2048))
        self.assertAlmostEqual(sum(w[3:]) / sum(w), 0.40, places=4)
        self.assertEqual(w[3:], [60_000, 80_000, 260_000])
        slots, w = p.schedule(1000 + 2_000_000_000, 2048)
        self.assertEqual(slots, round(0.25 * 2048))
        self.assertAlmostEqual(sum(w[3:]) / sum(w), 0.75, places=4)
        self.assertEqual(p.schedule(10**13, 2048), p.schedule(1000 + 2_000_000_000, 2048))

    def test_resume_merge(self):
        cfg = tiny_config()
        T.set_resumed(cfg, "practice", {"enabled": True, "pool": 16})
        self.assertTrue(cfg.practice.enabled)
        self.assertEqual(cfg.practice.pool, 16)
        self.assertEqual(cfg.practice.share, (0.40, 0.25))
        back = T.Config.from_dict(T.dataclasses.asdict(cfg))
        self.assertEqual(back.practice, cfg.practice)

    def test_old_config_has_practice_off(self):
        d = T.dataclasses.asdict(T.Config())
        del d["practice"]
        self.assertFalse(T.Config.from_dict(d).practice.enabled)


class Gae(unittest.TestCase):
    def test_boot_is_the_truncated_rows_next_value(self):
        torch.manual_seed(0)
        s, n = 6, 3
        r, v = torch.rand(s, n), torch.rand(s, n)
        done = torch.zeros(s, n, dtype=torch.bool)
        done[2, 1] = True
        combat = torch.rand(s, n) > 0.5
        last = torch.rand(n)
        boot = torch.zeros(s, n)
        boot[2, 1] = 0.7
        a0, _ = T.gae(r, v, done, combat, last, 0.95, 0.97)
        a1, _ = T.gae(r, v, done, combat, last, 0.95, 0.97, boot)
        same, _ = T.gae(r, v, done, combat, last, 0.95, 0.97, torch.zeros(s, n))
        self.assertTrue(torch.equal(same, a0))
        # Row 2 of env 1 gains 0.7, and so (by λ) do the rows before it in that run only.
        d = a1 - a0
        self.assertAlmostEqual(float(d[2, 1]), 0.7, places=6)
        self.assertTrue(torch.all(d[3:, 1] == 0))
        self.assertTrue(torch.all(d[:, 0] == 0) and torch.all(d[:, 2] == 0))
        self.assertTrue(torch.all(d[:2, 1] > 0))


class Envs(unittest.TestCase):
    def test_practice_episodes_and_their_final_position(self):
        env = Sts2RunEnvs(6, seed_range=(500, 100_000), combat="greedy", priv=True, events=True,
                          threads=2)
        self.assertTrue(env.has_practice)
        self.assertTrue((env.cell == 0).all())
        env.set_practice(slots=3, weights=[1] * 6, pool=16, max_uses=0, min_active=1, seed=1)
        rng = np.random.default_rng(0)
        truncs = 0
        for _ in range(400):
            legal = env.legal()
            env.step(np.where(legal, rng.random(legal.shape), -1).argmax(1))
            self.assertTrue((env.cell[3:] == 0).all())
            for i in np.nonzero(env.done)[0]:
                if env.end[i] == PRACTICE_END:
                    self.assertGreater(env.ended_cell[i], 0)
                    self.assertTrue(env.final_masks[i].any())
                    self.assertTrue(env.final_priv[i].any())
                    self.assertTrue(env.final_obs[i].any())
                    truncs += 1
        st = env.practice_stats()
        self.assertGreater(sum(st["started"]), 0, st)
        self.assertGreater(truncs, 0, st)
        env.set_practice(off=True)
        env.close()


class Learner(unittest.TestCase):
    def setUp(self):
        self.dir = pathlib.Path(tempfile.mkdtemp())

    def tearDown(self):
        shutil.rmtree(self.dir)

    def test_trains_with_practice_and_resumes(self):
        cfg = tiny_config(practice=dict(enabled=True, min_active=1, max_uses=0,
                                         share=[0.5, 0.5]))
        a = T.Learner(cfg, self.dir / "a")
        self.assertEqual(a.cfg.practice.start, 0)
        recs = [a.iterate() for _ in range(3)]
        self.assertTrue(all(r["finite"] for r in recs))
        self.assertIn("practice", recs[-1])
        self.assertEqual(recs[-1]["practice"]["slots"], 4)
        a.save()
        a.close()
        b = T.Learner(T.Config.from_dict(torch.load(self.dir / "a" / "state.pt",
                                                     weights_only=False)["config"]),
                      self.dir / "a", resume=True)
        self.assertEqual(b.cfg.practice.start, 0)
        self.assertNotIn(T.PRACTICE_SEED, b.env_seeds)
        self.assertTrue(b.iterate()["finite"])
        b.close()

    def test_batched_bootstrap_matches_per_step(self):
        """RLN26: the end-of-collect batched bootstrap equals one forward per truncating step
        (the old `_practice_ends`), on a small rollout. Greedy combat reaches the practice
        fights a random net never would, so the rollout drives the env by hand and queues its
        truncated episodes with the learner's own `_practice_ends`."""
        cfg = tiny_config(envs=16, steps=400, practice=dict(enabled=True, min_active=1, max_uses=0,
                                                           share=[0.5, 0.5]))
        a = T.Learner(cfg, self.dir / "a")
        torch.manual_seed(0)
        with torch.no_grad():  # the fresh value head is zero: make the values mean something
            for p in a.net.parameters():
                p.add_(0.05 * torch.randn_like(p))
        a.envs.close()
        env = a.envs = Sts2RunEnvs(16, seed_range=cfg.seed_range, character=cfg.character,
                                   ascension=cfg.ascension, combat="greedy", threads=1,
                                   priv=True, obs_v2=cfg.obs_v2, **T.E2E)
        env.set_practice(slots=8, weights=[1] * 6, pool=16, max_uses=0, min_active=1,
                         capture=True, seed=1)
        rng = np.random.default_rng(0)
        buf, out, pending = a._alloc(), {}, []
        for t in range(cfg.steps):
            legal = env.legal()
            env.step(np.where(legal, rng.random(legal.shape), -1).argmax(1))
            done = env.done.astype(bool)
            if done.any():
                a._practice_ends(buf, t, done, env.ended_cell.copy(), out, pending)
        self.assertGreater(len({t for t, *_ in pending}), 1,
                           "the rollout needs truncations at several steps")
        want = torch.zeros_like(buf["boot"])
        with torch.no_grad():
            for t, idx, obs, masks, priv in pending:
                _, _, _, v = a._forward(*(torch.from_numpy(x).to(a.dev)
                                          for x in (obs, masks, priv)), net=a.net)
                want[t, torch.from_numpy(idx)] = v.float()
            got = torch.zeros_like(want)
            a._practice_bootstrap(dict(boot=buf["boot"]), pending, a.net)
            a._practice_bootstrap(dict(boot=got), pending, a.net, chunk=3)
        self.assertTrue(buf["trunc"].any() if "trunc" in buf else True)
        n = int((want != 0).sum())
        self.assertGreater(n, 1)
        diff = float((buf["boot"] - want).abs().max())
        print(f"\nbootstrap: {n} values, max abs diff batched vs per-step {diff:.3e}")
        self.assertLess(diff, 1e-5)
        self.assertTrue(torch.equal(buf["boot"] != 0, want != 0))
        self.assertLess(float((got - want).abs().max()), 1e-5)  # chunked forwards too
        a.close()

    def test_an_empty_mask_row_leaves_the_update(self):
        """RLN18: an episode that ends at its reset (a practice start whose fight is over before
        its first decision) leaves one row with a zero mask, sampled from all -inf logits (a
        NaN log-prob). With it in the update, pg was NaN (NECRO1, 13:28)."""
        def batch():
            torch.manual_seed(0)
            a = T.Learner(tiny_config(), self.dir / "e")
            a.iterate()  # past the probe and warm-up's first iteration
            buf, _ = a.collect()
            buf["packed"] = {k: v.clone() for k, v in buf["packed"].items()}
            buf["masks"][3, 5] = 0
            buf["logp"][3, 5] = float("nan")
            buf["done"][3, 5] = True
            return a, buf

        a, buf = batch()
        out = a.update(buf)
        self.assertEqual(out["rows_empty"], 1)
        self.assertTrue(out["finite"], out)
        a.close()
        # The same batch without the drop is the NaN of the report.
        b, buf = batch()
        b._drop_empty = lambda flat, rows: (flat, rows, 0)
        self.assertFalse(math.isfinite(b.update(buf)["pg"]))
        b.close()

    def test_a_practice_off_checkpoint_resumes_with_practice_on(self):
        a = T.Learner(tiny_config(), self.dir / "c")
        a.iterate()
        a.iterate()
        a.save()
        dec = a.decisions
        a.close()
        cfg = T.Config.from_dict(torch.load(self.dir / "c" / "state.pt",
                                            weights_only=False)["config"])
        self.assertFalse(cfg.practice.enabled)
        T.set_resumed(cfg, "practice", {"enabled": True, "min_active": 1})
        b = T.Learner(cfg, self.dir / "c", resume=True)
        self.assertEqual(b.cfg.practice.start, dec)
        rec = b.iterate()
        self.assertTrue(rec["finite"])
        self.assertEqual(rec["iteration"], 3)
        b.save()
        state = torch.load(self.dir / "c" / "state.pt", weights_only=False)
        self.assertEqual(state["config"]["practice"]["start"], dec)
        b.close()


if __name__ == "__main__":
    unittest.main()
