"""RLN44: the fast-PPO flags and the practice HP-lost reward (`tasks/RLN44-fast-ppo.md`).

On the CPU (tiny nets), plus one CUDA-only check (`compile_encode`):

* `encode.legal_counts` is `mask_bits(...).sum(1)` on the host;
* `static_head` (pairs by `nonzero_static`, the run head by `torch.where`) gives the same pairs
  and combat logits bit for bit as the sync head, the run rows' and the gradients to float
  noise;
* `fast_gather` gives the same forward bit for bit and the same gradients to float noise;
* with every RLN44 flag off a learner trains to the same bits as one built without them (the
  defaults); `fixed_probe=False` under `kl_source="batch"` trains to the same bits as with it;
  all the flags on train to float noise of all off; `epochs` is settable at a resume;
* `practice.hp_coef`: 0 is bit-identical; `hp_fraction` reads the live combat HP or the
  header's; on, a practice episode's shaped rewards add up to c * (phi(end) - phi(start)).
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
from sts2.e2e import encode as E  # noqa: E402
from sts2.e2e.net import E2ENet, masked_stats  # noqa: E402
from test_e2e_full import full_states  # noqa: E402
from test_e2e_learner import tiny_config  # noqa: E402

FULL = dict(obs_v2=True, obs_full=True, card_stats=True, discard_potion=True, run_head=True)


def full_batch(n=256):
    obs, masks, priv = full_states()
    obs, masks, priv = obs[:n], masks[:n], priv[:n]
    tok = E.encode(obs, masks, v2=True, full=True, discard=True)
    return tok, E.encode_priv(priv), masks


def loss_grads(net, tok, ptok):
    net.zero_grad(set_to_none=True)
    logits, v = net(tok, ptok)
    logp, ent = masked_stats(logits.float())
    pick = torch.where(torch.isfinite(logp), logp, torch.zeros_like(logp)).sum(1)
    (pick.sum() + ent.sum() + v.sum()).backward()
    return logits.detach(), v.detach(), {n: p.grad.clone() for n, p in net.named_parameters()
                                         if p.grad is not None}


class Head(unittest.TestCase):
    def test_legal_counts_match_the_mask_bits(self):
        _, _, masks = full_batch()
        for discard in (False, True):
            want = E.mask_bits(masks, discard).sum(1).numpy()
            self.assertTrue(np.array_equal(E.legal_counts(masks.numpy(), discard), want))

    def test_static_head_is_the_same_function(self):
        tok, ptok, masks = full_batch()
        torch.manual_seed(0)
        net = E2ENet(d=32, layers=1, **{k: v for k, v in FULL.items() if k != "obs_events"})
        with torch.no_grad():  # the zero-initialised outputs would hide a mismatch
            for p in net.parameters():
                p.add_(0.05 * torch.randn_like(p))
        self.assertTrue((~tok["combat"]).any() and tok["combat"].any())
        a_logits, a_v, a_g = loss_grads(net, tok, ptok)
        stok = dict(tok, n_legal=int(E.legal_counts(masks.numpy(), True).sum()))
        b_logits, b_v, b_g = loss_grads(net, stok, ptok)
        # The same pairs and the combat rows' logits bit for bit; the run head now scores every
        # pair (a larger matmul), so the run rows' agree to float noise.
        c = tok["combat"]
        self.assertTrue(torch.equal(torch.isfinite(a_logits), torch.isfinite(b_logits)))
        self.assertTrue(torch.equal(a_logits[c], b_logits[c]))
        torch.testing.assert_close(b_logits[~c], a_logits[~c], rtol=1e-6, atol=1e-6)
        self.assertTrue(torch.equal(a_v, b_v))
        self.assertEqual(a_g.keys(), b_g.keys())
        for k in a_g:
            torch.testing.assert_close(a_g[k], b_g[k], rtol=1e-5, atol=1e-6, msg=k)

    def test_fast_gather_is_the_same_function(self):
        tok, ptok, _ = full_batch()
        torch.manual_seed(0)
        net = E2ENet(d=32, layers=1, **{k: v for k, v in FULL.items() if k != "obs_events"})
        with torch.no_grad():
            for p in net.parameters():
                p.add_(0.05 * torch.randn_like(p))
        a_logits, a_v, a_g = loss_grads(net, tok, ptok)
        net.fast_gather = True
        b_logits, b_v, b_g = loss_grads(net, tok, ptok)
        self.assertTrue(torch.equal(a_logits, b_logits))
        self.assertTrue(torch.equal(a_v, b_v))
        for k in a_g:
            torch.testing.assert_close(a_g[k], b_g[k], rtol=1e-5, atol=1e-6, msg=k)

    @unittest.skipUnless(torch.cuda.is_available(), "compile_encode is for CUDA")
    def test_compiled_encode_matches_eager(self):
        obs, masks, _ = full_states()
        obs, masks = obs[:512].cuda(), masks[:512].cuda()
        kw = dict(v2=True, full=True, discard=True)
        want = E.encode(obs, masks, **kw)
        got = torch.compile(E.encode, dynamic=False)(obs, masks, **kw)
        self.assertEqual(want.keys(), got.keys())
        for k, x in want.items():
            if not torch.is_tensor(x):
                self.assertEqual(x, got[k])
            elif x.dtype.is_floating_point:
                torch.testing.assert_close(got[k], x, rtol=1e-6, atol=1e-6, msg=k)
            else:
                self.assertTrue(torch.equal(got[k], x), k)


def weights(learner):
    return [p.detach().clone() for p in learner.net.parameters()]


class Learner(unittest.TestCase):
    def setUp(self):
        self.dir = pathlib.Path(tempfile.mkdtemp())

    def tearDown(self):
        shutil.rmtree(self.dir)

    def train(self, name, iters=2, **kw):
        a = T.Learner(tiny_config(**kw), self.dir / name)
        recs = [a.iterate() for _ in range(iters)]
        w = weights(a)
        a.close()
        return w, recs

    def test_flags_off_are_the_defaults(self):
        off = dict(fast_gather=False, fixed_probe=True, static_head=False, compile_encode=False,
                   pinned_copies=False, compile_net=False)
        base = T.dataclasses.asdict(T.Config())
        self.assertTrue(all(base[k] == v for k, v in off.items()))
        self.assertEqual(base["practice"]["hp_coef"], 0.0)

    def test_no_fixed_probe_trains_the_same_bits(self):
        a, ra = self.train("a", **FULL, kl_source="batch")
        b, rb = self.train("b", **FULL, kl_source="batch", fixed_probe=False)
        self.assertTrue(all(torch.equal(p, q) for p, q in zip(a, b)))
        self.assertEqual([r["kl_batch"] for r in ra], [r["kl_batch"] for r in rb])
        self.assertTrue(all(r["kl"] != r["kl"] for r in rb))  # NaN: not computed
        with self.assertRaises(ValueError):
            self.train("c", fixed_probe=False, kl_source="fixed")

    def test_speed_flags_train_to_float_noise(self):
        a, _ = self.train("a", **FULL, kl_source="batch", epochs=1)
        b, _ = self.train("b", **FULL, kl_source="batch", epochs=1, fixed_probe=False,
                          fast_gather=True, static_head=True, pinned_copies=True)
        # Adam turns float noise in a near-zero gradient into a step of up to lr (1e-4): the
        # weights agree within one step; a wrong gradient would move them by many.
        for p, q in zip(a, b):
            torch.testing.assert_close(p, q, rtol=1e-3, atol=1e-4)

    def test_epochs_is_settable_at_a_resume(self):
        self.assertIn("epochs", T.RESUME_SETTABLE)
        cfg = tiny_config(epochs=2)
        T.set_resumed(cfg, "epochs", 1)
        self.assertEqual(cfg.epochs, 1)
        a = T.Learner(tiny_config(epochs=1), self.dir / "e")
        self.assertEqual(a.iterate()["epochs"], 1)
        a.close()


class HpReward(unittest.TestCase):
    def setUp(self):
        self.dir = pathlib.Path(tempfile.mkdtemp())

    def tearDown(self):
        shutil.rmtree(self.dir)

    def test_hp_fraction_reads_the_live_hp_in_a_fight(self):
        obs = np.zeros((3, E.COMBAT_AT + 4), np.uint8)
        obs[:, T.HEADER_HP:T.HEADER_HP + 4] = np.frombuffer(np.array([50, 80], "<i2").tobytes(),
                                                            np.uint8)
        obs[1, E.COMBAT_AT:E.COMBAT_AT + 4] = np.frombuffer(np.array([20, 80], "<i2").tobytes(),
                                                            np.uint8)
        obs[2, T.HEADER_HP:T.HEADER_HP + 4] = 0  # no max HP: 0, never a division by zero
        np.testing.assert_allclose(T.hp_fraction(obs), [50 / 80, 20 / 80, 0.0])

    def practice(self, c):
        return dict(enabled=True, min_active=1, max_uses=0, share=[0.5, 0.5], hp_coef=c)

    def test_zero_is_bit_identical(self):
        def run(name, **pr):
            a = T.Learner(tiny_config(steps=96, practice=pr), self.dir / name)
            recs = [a.iterate() for _ in range(2)]
            w = weights(a)
            a.close()
            return w, recs
        a, ra = run("a", **{k: v for k, v in self.practice(0.0).items() if k != "hp_coef"})
        b, rb = run("b", **self.practice(0.0))
        self.assertTrue(all(torch.equal(p, q) for p, q in zip(a, b)))
        self.assertEqual([r["reward_sum"] for r in ra], [r["reward_sum"] for r in rb])

    def test_practice_episodes_telescope(self):
        """A collect with c > 0 against c = 0 from the same start: the difference is zero on
        normal-run rows and sums, per finished practice episode, to c * (phi(end) - phi(start))."""
        c = 0.5
        out = {}
        for name, coef in (("off", 0.0), ("on", c)):
            a = T.Learner(tiny_config(envs=8, steps=300, practice=self.practice(0.0)),
                          self.dir / name)
            a.iterate()  # captures snapshots: practice starts from the second collect on
            a.cfg.practice.hp_coef = coef  # after it, so both collect with the same weights
            a._practice_apply()
            buf = a._alloc()
            envs = a.envs
            cells, phis, ends = [], [], []
            step0 = envs.step

            def step(actions):  # record each row's practice flag and phi before and after
                cells.append(envs.cell.copy())
                phis.append(T.hp_fraction(envs.obs))
                step0(actions)
                after = np.where(envs.done.astype(bool), T.hp_fraction(envs.final_obs),
                                 T.hp_fraction(envs.obs))
                died = envs.done.astype(bool) & (envs.end == T.RUN_ENDS.index("died"))
                ends.append((np.where(died, 0.0, after), envs.done.astype(bool).copy()))
            envs.step = step
            a._collect_into(buf, a.net, a.gen)
            envs.step = step0
            out[name] = (buf["reward"].numpy().astype(np.float64), np.stack(cells),
                         np.stack(phis), ends)
            a.close()
        r_off, cells, phi0, ends = out["off"]
        r_on, cells_on, _, _ = out["on"]
        self.assertTrue(np.array_equal(cells, cells_on))  # the same trajectory (same seeds)
        diff = r_on - r_off
        prac = cells != 0
        self.assertTrue(prac.any())
        self.assertTrue(np.all(diff[~prac] == 0))
        # Per row: c * (phi after - phi before) on practice rows.
        phi1 = np.stack([e[0] for e in ends])
        np.testing.assert_allclose(diff[prac], (c * (phi1 - phi0))[prac], atol=1e-6)
        # Per finished practice episode: the sum telescopes to its end minus its start.
        done = np.stack([e[1] for e in ends])
        checked = 0
        for e in range(cells.shape[1]):
            start = None
            for t in range(cells.shape[0]):
                if prac[t, e] and start is None:
                    start = t
                if start is not None and done[t, e]:
                    want = c * (phi1[t, e] - phi0[start, e])
                    self.assertAlmostEqual(diff[start:t + 1, e].sum(), want, places=5)
                    checked += 1
                    start = None
        self.assertGreater(checked, 0)


if __name__ == "__main__":
    unittest.main()
