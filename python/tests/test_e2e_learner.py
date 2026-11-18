"""The e2e encoder and learner (`docs/plans/necro-e2e.md`, WP3 and WP4 done-checks):
`python -m unittest discover -s python/tests` (after `cargo build --profile ffi -p sts2-ffi`).

On the CPU (tiny nets), so they run anywhere: the encoder is bit-stable across batch sizes;
chained GAE matches a reference and chains across fights and the run but not across runs; the
rewards of a run add up to its event totals; logits are -inf exactly off the mask, samples are
legal and the class entropy is in [0, 1]; the privileged input never reaches the policy (H3:
no gradient, logits bit-identical under permuted or NaN-filled priv); and a learner killed
after an iteration and resumed reproduces the next 3 iterations to the bit, with the speed
flags on, with overlap (the batch collected ahead is collected again) and with all flags off. The packed token
cache gives back `encode`'s tensors; the value block's first-position path is the same
function; with overlap each batch's logprobs are the pre-update weights'; and a checkpoint
saved without overlap resumes with it.
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
from sts2 import E2E, Sts2RunEnvs  # noqa: E402
from sts2.e2e import encode as E  # noqa: E402
from sts2.e2e.net import E2ENet, masked_stats  # noqa: E402

CPU = torch.device("cpu")


def smoke_states(n=600, envs=32, priv=False, seed=0):
    env = Sts2RunEnvs(envs, seed_range=(23_000_000, 23_100_000), ascension=10, threads=2,
                      priv=priv, **E2E)
    rng = np.random.default_rng(seed)
    obs, masks, privs = [], [], []
    while sum(len(o) for o in obs) < n:
        obs.append(env.obs.copy())
        masks.append(env.masks.view(np.int64).copy())
        privs.append(env.priv.copy())
        legal = env.legal()
        env.step(np.where(legal, rng.random(legal.shape), -1).argmax(1))
    env.close()
    cat = lambda xs: torch.from_numpy(np.concatenate(xs)[:n])  # noqa: E731
    return cat(obs), cat(masks), cat(privs)


def tiny_config(**kw):
    base = dict(envs=8, threads=1, steps=8, minibatch=32, micro=16, d=32, layers=1,
                probe_rows=16, warmup_iters=2, device="cpu", deterministic=True,
                seed_range=(300_000_000, 400_000_000), ckpt_every=10**12)
    base.update(kw)
    return T.Config.from_dict({**T.dataclasses.asdict(T.Config()), **base})


class Encoder(unittest.TestCase):
    def test_bit_stable_and_lean(self):
        obs, masks, priv = smoke_states(priv=True)
        full = E.encode(obs, masks)
        for size in (1, 5, 77):
            parts = [E.encode(obs[i:i + size], masks[i:i + size])
                     for i in range(0, len(obs), size)]
            for k, v in full.items():
                if v.dim() >= 1 and v.shape[0] == len(obs):
                    self.assertTrue(torch.equal(torch.cat([p[k] for p in parts]), v), k)
        self.assertLessEqual(float(full["count"].float().mean()), 64)
        self.assertFalse(full["overflow"].any())
        # Every legal index's entity is a valid token of its row.
        rows, acts = full["legal"].nonzero(as_tuple=True)
        ent = full["ent"][rows, acts]
        self.assertTrue(full["valid"][rows, ent].all())
        p = E.encode_priv(priv)
        self.assertEqual(p["kind"].shape, (len(obs), E.PRIV_T))

    def test_pack_round_trip(self):
        obs, masks, priv = smoke_states(n=400, priv=True, seed=1)
        tok, ptok = E.encode(obs, masks), E.encode_priv(priv)
        # The columns `pack` leaves out are zero.
        self.assertFalse(tok["num"][:, 1:, E.TOK_NUM:].any())
        self.assertFalse(ptok["num"][:, :, E.PRIV_NUM:].any())
        t = int(tok["width"].max())
        got, pgot = E.unpack(E.pack(tok, ptok), masks, t)
        self.assertEqual(got.pop("t"), t)
        for k, v in got.items():
            want = tok[k][:, :t] if k in ("valid", "kind", "ident", "cat", "owner", "num") \
                else tok[k]
            self.assertTrue(torch.equal(v, want) and v.dtype == want.dtype, k)
        for k, v in pgot.items():
            self.assertTrue(torch.equal(v, ptok[k]) and v.dtype == ptok[k].dtype, k)

    def test_value_first_is_the_same_function(self):
        obs, masks, priv = smoke_states(n=200, priv=True, seed=2)
        torch.manual_seed(0)
        net = E2ENet(d=32, layers=1)
        for p in net.parameters():
            torch.nn.init.normal_(p, std=0.1)
        tok, ptok = E.encode(obs, masks), E.encode_priv(priv)
        with torch.no_grad():
            base_l, base_v = net(tok, ptok)
            net.value_first = True
            got_l, got_v = net(tok, ptok)
        self.assertTrue(torch.equal(got_l, base_l))
        torch.testing.assert_close(got_v, base_v, rtol=0, atol=1e-6)


class Gae(unittest.TestCase):
    def reference(self, r, v, d, c, last, lc, lr):
        steps, n = r.shape
        adv = np.zeros((steps, n))
        for i in range(n):
            a, nv = 0.0, last[i]
            for t in range(steps - 1, -1, -1):
                live = 0.0 if d[t, i] else 1.0
                delta = r[t, i] + live * nv - v[t, i]
                a = delta + (lc if c[t, i] else lr) * live * a
                adv[t, i] = a
                nv = v[t, i]
        return adv

    def test_matches_reference_and_chains(self):
        g = np.random.default_rng(0)
        s, n = 40, 6
        r = g.random((s, n)) * (g.random((s, n)) < 0.2)
        v = g.random((s, n))
        d = g.random((s, n)) < 0.05
        c = g.random((s, n)) < 0.7
        last = g.random(n)
        t = lambda x: torch.tensor(x, dtype=torch.float64)  # noqa: E731
        adv, ret = T.gae(t(r), t(v), torch.tensor(d), torch.tensor(c), t(last), 0.95, 0.97)
        ref = self.reference(r, v, d, c, last, 0.95, 0.97)
        np.testing.assert_allclose(adv.numpy(), ref, rtol=0, atol=1e-12)
        np.testing.assert_allclose(ret.numpy(), ref + v, atol=1e-12)
        # A run row's advantage chains through the fight after it: change a later combat
        # reward and the run row moves; a reward after the run's end moves nothing before it.
        r1 = np.zeros((4, 1))
        c1 = np.array([[False], [True], [True], [False]])
        d1 = np.array([[False], [False], [True], [False]])
        base, _ = T.gae(t(r1), t(np.zeros((4, 1))), torch.tensor(d1), torch.tensor(c1),
                        t(np.zeros(1)), 0.95, 0.97)
        r2 = r1.copy()
        r2[2, 0] = 1.0
        a2, _ = T.gae(t(r2), t(np.zeros((4, 1))), torch.tensor(d1), torch.tensor(c1),
                      t(np.zeros(1)), 0.95, 0.97)
        self.assertAlmostEqual(float(a2[0, 0] - base[0, 0]), 0.97 * 0.95)
        r3 = r1.copy()
        r3[3, 0] = 1.0
        a3, _ = T.gae(t(r3), t(np.zeros((4, 1))), torch.tensor(d1), torch.tensor(c1),
                      t(np.zeros(1)), 0.95, 0.97)
        self.assertTrue(torch.equal(a3[:3], base[:3]))


class Learner(unittest.TestCase):
    def setUp(self):
        self.dir = pathlib.Path(tempfile.mkdtemp(dir=pathlib.Path(__file__).resolve()
                                                 .parents[2] / "target"))

    def tearDown(self):
        shutil.rmtree(self.dir, ignore_errors=True)

    def test_rewards_add_up_to_the_run(self):
        """The learner's per-row rewards are the events' (0.002 per floor, 0.05 per boss, 1 a
        win), and over a finished run they add up to its totals: the floors to its final floor
        less its first row's."""
        cfg = tiny_config(steps=64, envs=4)
        learner = T.Learner(cfg, self.dir / "r")
        buf, runs = learner.collect()
        learner.close()
        rew = buf["reward"].numpy()
        env = Sts2RunEnvs(cfg.envs, seed_range=cfg.seed_range, ascension=10, priv=True, **E2E)
        total = np.zeros(cfg.envs)
        floors = np.zeros(cfg.envs)
        bosses = np.zeros(cfg.envs)
        start = env.obs[:, 7].astype(float)
        ended = 0
        for t in range(cfg.steps):
            env.step(buf["action"][t].numpy())
            ev = env.events.astype(float)
            total += rew[t]
            floors += ev[:, 0]
            bosses += ev[:, 1]
            for i in np.nonzero(env.done)[0]:
                won = float(env.won[i])
                self.assertEqual(floors[i], env.floor[i] - start[i])
                self.assertAlmostEqual(total[i], 0.002 * floors[i] + 0.05 * bosses[i] + won,
                                       places=5)
                ended += 1
                total[i] = floors[i] = bosses[i] = 0
                start[i] = env.obs[i, 7]
        self.assertGreater(ended, 0)
        self.assertEqual(ended, len(runs["floors"]))
        env.close()

    def test_masking_entropy_and_priv_isolation(self):
        obs, masks, priv = smoke_states(n=200, priv=True)
        torch.manual_seed(0)
        net = E2ENet(d=32, layers=1)
        for p in net.parameters():  # break the zero-init heads so logits differ
            torch.nn.init.normal_(p, std=0.1)
        tok = E.encode(obs, masks)
        ptok = E.encode_priv(priv)
        logits, value = net(tok, ptok)
        self.assertTrue(torch.equal(torch.isfinite(logits), tok["legal"]))
        logp, ent = masked_stats(logits)
        self.assertTrue(((ent >= -1e-6) & (ent <= 1 + 1e-6)).all())
        self.assertTrue((tok["legal"].sum(1) >= 2).all())
        a = torch.distributions.Categorical(logits=logits).sample()
        self.assertTrue(tok["legal"][torch.arange(len(a)), a].all())
        # H3: no gradient from the policy into anything privileged.
        value_ids = {id(p) for p in net.value_parameters()}
        self.assertFalse(value_ids & {id(p) for p in net.policy_parameters()})
        net.zero_grad()
        lg, _, _ = net.policy(tok)
        lg[torch.isfinite(lg)].sum().backward()
        for name, p in net.named_parameters():
            if id(p) in value_ids:
                self.assertTrue(p.grad is None or not p.grad.any(), name)
        # The logits are the same bits with priv permuted across rows or NaN-poisoned.
        perm = E.encode_priv(priv[torch.randperm(len(priv))])
        nan = {k: v for k, v in ptok.items()}
        nan["num"] = torch.full_like(ptok["num"], float("nan"))
        with torch.no_grad():
            base, _ = net(tok, ptok)
            for other in (perm, nan):
                got, _ = net(tok, other)
                self.assertTrue(torch.equal(got, base))
        # The value does read priv.
        with torch.no_grad():
            _, v2 = net(tok, perm)
        self.assertFalse(torch.equal(v2, value.detach()))

    def test_kill_and_resume_reproduce_the_next_iterations(self):
        self.kill_and_resume(tiny_config())

    def test_kill_and_resume_with_overlap(self):
        self.kill_and_resume(tiny_config(overlap=True))

    def test_kill_and_resume_without_the_speed_flags(self):
        self.kill_and_resume(tiny_config(overlap=False, sort_micro=False, cache_tokens=False,
                                         value_first=False, fused_adam=False, probe_chunk=0))

    def test_overlap_collects_with_the_weights_before_the_update(self):
        """The batch collected during update N holds the logprobs and values of the weights
        before update N (one iteration of policy lag), and those are what it trains on."""
        cfg = tiny_config(overlap=True)
        a = T.Learner(cfg, self.dir / "o")
        a.iterate()
        before = [p.detach().clone() for p in a.net.parameters()]
        a.iterate()
        buf = a.pending[0]
        net = E2ENet(d=cfg.d, layers=cfg.layers)
        with torch.no_grad():
            for p, q in zip(net.parameters(), before):
                p.copy_(q)
            flat = {k: v.flatten(0, 1) for k, v in buf["packed"].items()}
            tok, ptok = E.unpack(flat, buf["masks"].flatten(0, 1),
                                 int(buf["width"].max()))
            logits, v = net(tok, ptok)
        logp = torch.log_softmax(logits, -1).gather(1, buf["action"].flatten().unsqueeze(1))
        torch.testing.assert_close(logp.squeeze(1), buf["logp"].flatten(), rtol=0, atol=1e-5)
        torch.testing.assert_close(v, buf["value"].flatten(), rtol=0, atol=1e-5)
        self.assertFalse(all(torch.equal(p, q) for p, q in zip(a.net.parameters(), before)))
        a.close()

    def test_an_old_checkpoint_resumes_with_overlap(self):
        """A checkpoint saved without overlap (as NECRO1's) resumes with it on: its own
        weights collect the first batch, and it trains on."""
        old = tiny_config(overlap=False)
        a = T.Learner(old, self.dir / "c")
        a.iterate()
        a.save()
        state = torch.load(self.dir / "c" / "state.pt", weights_only=False)
        self.assertNotIn("behaviour", state)
        a.close()
        b = T.Learner(tiny_config(overlap=True), self.dir / "c", resume=True)
        for p, q in zip(b.behaviour.parameters(), b.net.parameters()):
            self.assertTrue(torch.equal(p, q))
        rec = b.iterate()
        self.assertEqual(rec["iteration"], 2)
        self.assertTrue(rec["finite"])
        b.close()

    def kill_and_resume(self, cfg):
        a = T.Learner(cfg, self.dir / "a")
        a.iterate()
        a.iterate()
        a.save(self.dir / "a" / "state.pt")
        ref = [a.iterate() for _ in range(3)]
        ref_params = [p.detach().clone() for p in a.net.parameters()]
        ref_obs = a.envs.obs.copy()
        a.close()
        (self.dir / "b").mkdir()
        shutil.copy(self.dir / "a" / "state.pt", self.dir / "b" / "state.pt")
        b = T.Learner(cfg, self.dir / "b", resume=True)
        got = [b.iterate() for _ in range(3)]
        timing = ("collect_dps", "update_dps", "total_dps", "gpu_mem_gb")
        nan = lambda v: "nan" if isinstance(v, float) and math.isnan(v) else v  # noqa: E731
        strip = lambda r: {k: nan(v) for k, v in r.items() if k not in timing}  # noqa: E731
        self.assertEqual([strip(r) for r in got], [strip(r) for r in ref])
        for p, q in zip(b.net.parameters(), ref_params):
            self.assertTrue(torch.equal(p.detach(), q))
        self.assertTrue((b.envs.obs == ref_obs).all())
        b.close()


if __name__ == "__main__":
    unittest.main()
