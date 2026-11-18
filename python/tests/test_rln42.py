"""RLN42: the warm start, the run head and the KL to the prior (on run decisions only).

    PYTHONPATH=<venv site-packages> python -m pytest -q python/tests/test_rln42.py   (after the ffi build)
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

import rln41_prior as P  # noqa: E402
import rln42_prior as R  # noqa: E402
import train_e2e as T  # noqa: E402
from sts2.e2e.net import E2ENet  # noqa: E402
from test_e2e_rln14 import tiny_config  # noqa: E402

RESET = ["run_head.", "v_hidden.", "v_out."]
HAVE_PRIOR = (P.DATA / "prior.pt").exists()


class WarmStart(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.dir = pathlib.Path(tempfile.mkdtemp())
        src = T.Learner(tiny_config(), cls.dir / "src")
        src.iterate()
        src.save()
        src.close()
        cls.src = torch.load(cls.dir / "src" / "state.pt", weights_only=False)

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.dir, ignore_errors=True)

    def warm(self, **kw):
        cfg = tiny_config(run_head=True, init_from=str(self.dir / "src" / "state.pt"),
                          init_reset=RESET, **kw)
        learner = T.Learner(cfg, self.dir / "dst")
        self.addCleanup(learner.close)
        self.addCleanup(shutil.rmtree, self.dir / "dst", True)
        return learner

    def source(self, weights):
        names = [n for n, _ in T.net_of(T.Config.from_dict(self.src["config"])).named_parameters()]
        if weights == "raw":
            return dict(self.src["net"])
        return dict(zip(names, self.src["emas"][1]))  # ema (0.996, 0.999)

    def test_kept_equal_reset_differ_and_counters_fresh(self):
        for weights in ("ema0.999", "raw"):
            learner = self.warm(init_weights=weights)
            src = self.source(weights)
            kept = reset = 0
            for name, p in learner.net.named_parameters():
                if name.startswith(tuple(RESET)):
                    reset += 1
                    if name in src:  # v_hidden, v_out: re-initialised, not the source's
                        self.assertFalse(torch.equal(p.detach().cpu(), src[name]), name)
                else:
                    kept += 1
                    self.assertTrue(torch.equal(p.detach().cpu(), src[name]), name)
            self.assertGreater(kept, 20)
            self.assertGreaterEqual(reset, 10)
            self.assertEqual(learner.decisions, 0)
            self.assertEqual(learner.iteration, 0)
            self.assertEqual(len(learner.opt.state), 0)
            self.assertIsNone(learner.probe)
            for ema in learner.emas:  # the EMAs start at the loaded weights
                for e, p in zip(ema, learner.net.parameters()):
                    self.assertTrue(torch.equal(e, p.detach()))
            learner.close()
            shutil.rmtree(self.dir / "dst", ignore_errors=True)

    def test_a_missing_parameter_must_be_listed(self):
        cfg = tiny_config(run_head=True, init_from=str(self.dir / "src" / "state.pt"),
                          init_reset=["v_out."])
        with self.assertRaisesRegex(ValueError, "run_head"):
            T.warm_start(T.net_of(cfg), cfg)

    def test_trains_after_the_warm_start(self):
        rec = self.warm().iterate()
        self.assertTrue(rec["finite"])
        self.assertEqual(rec["decisions"], 8 * 8)


class RunHead(unittest.TestCase):
    def test_off_is_the_same_parameter_list(self):
        a = [n for n, _ in E2ENet(d=32, layers=1).named_parameters()]
        b = [n for n, _ in E2ENet(d=32, layers=1, run_head=True).named_parameters()]
        self.assertEqual(b[:len(a)], a)
        self.assertTrue(all(n.startswith("run_head.") for n in b[len(a):]))

    def test_combat_rows_use_the_shared_head_and_run_rows_their_own(self):
        torch.manual_seed(0)
        on = E2ENet(d=32, layers=1, run_head=True)
        off = E2ENet(d=32, layers=1)
        off.load_state_dict({k: v for k, v in on.state_dict().items() if not k.startswith("run_head.")})
        torch.nn.init.normal_(on.run_head.pout.weight, std=0.5)
        b, t, a = 6, 5, on.n_actions
        h = torch.randn(b, t, 32)
        legal = torch.zeros(b, a, dtype=torch.bool)
        legal[:, [3, 150, 236, 237]] = True
        tok = dict(legal=legal, ent=torch.randint(0, t, (b, a)), tgt=torch.full((b, a), -1),
                   akind=torch.randint(0, 32, (a,)), asub=torch.randint(0, 16, (a,)),
                   combat=torch.tensor([True, False, True, False, False, True]))
        lo, lf = on.head(h, tok), off.head(h, tok)
        c = tok["combat"]
        self.assertTrue(torch.equal(lo[c], lf[c]))
        self.assertFalse(torch.allclose(lo[~c][legal[~c]], lf[~c][legal[~c]]))
        self.assertTrue(torch.equal(on.head(h, tok, run=False), lf))


class KlMasking(unittest.TestCase):
    def table(self, a2o, lp, kind):
        kl = R.PriorKL.__new__(R.PriorKL)
        kl.cfg, kl.dev, kl.base = R.PriorCfg(), torch.device("cpu"), 0
        kl.next_id = len(kind)
        kl.scale = torch.tensor([kl.cfg.kind_scale.get(k, 1.0) for k in R.KINDS])
        kl.leave = torch.tensor([kl.cfg.leave if k in R.LEAVE_KINDS else 0.0 for k in R.KINDS])
        kl.a2o = torch.as_tensor(np.stack(a2o), dtype=torch.int8)
        pad = torch.full((len(lp), R.K_MAX), float("-inf"))
        for i, x in enumerate(lp):
            pad[i, :len(x)] = torch.log(torch.tensor(x))
        kl.lp, kl.kind = pad, torch.tensor(kind)
        return kl

    def setUp(self):
        rest = np.full(R.N_ACT, -1, np.int8)
        rest[[236, 237]] = [0, 1]  # HEAL, SMITH
        shop = np.full(R.N_ACT, -1, np.int8)
        shop[[333, 334, 354]] = [0, 1, 2]
        self.kl = self.table([rest, shop], [[0.2, 0.8], [0.5, 0.3, 0.2]],
                             [R.KIND_ID["rest"], R.KIND_ID["shop_card"]])

    def logits(self):
        x = torch.full((4, R.N_ACT), float("-inf"))
        x[:, [10, 236, 237, 333, 334, 354, 300]] = torch.randn(4, 7)
        return x.requires_grad_(True)

    def test_only_mapped_run_rows_get_a_term(self):
        torch.manual_seed(1)
        x = self.logits()
        pent = torch.tensor([[-1, -1, -1], [0, -1, -1], [-1, -1, -1], [1, -1, -1]])  # 0, 2: none
        out, (kind, kl) = self.kl.kl(torch.log_softmax(x, -1), pent, 0.1)
        self.assertEqual(out[0].item(), 0.0)
        self.assertEqual(out[2].item(), 0.0)
        self.assertGreater(out[1].item(), 0)
        self.assertAlmostEqual(out[3].item(), 0.1 * 0.25 * kl[1].item(), places=6)  # shop_card / 4
        out.sum().backward()
        g = x.grad
        self.assertTrue(torch.all(g[0] == 0) and torch.all(g[2] == 0))  # combat / unmapped rows
        # Outside the entry's options (10, 300 and the shop's actions on the rest row) no gradient.
        for a in (10, 300, 333, 334, 354):
            self.assertLess(abs(g[1, a].item()), 1e-7)
        for a in (10, 300, 236, 237):
            self.assertLess(abs(g[3, a].item()), 1e-7)

    def test_zero_when_the_policy_matches_the_prior(self):
        x = torch.full((1, R.N_ACT), float("-inf"))
        x[0, 236], x[0, 237], x[0, 10] = np.log(0.2), np.log(0.8), 3.0  # 10: a map move, outside
        out, _ = self.kl.kl(torch.log_softmax(x, -1), torch.tensor([[0, -1, -1]]), 0.1)
        self.assertLess(abs(out.item()), 1e-6)

    def test_backing_out_of_a_select_costs_leave(self):
        sel = np.full(R.N_ACT, -1, np.int8)
        sel[[300, 301]] = [0, 1]  # two cards; 331 (cancel) is outside the options
        kl = self.table([sel], [[0.5, 0.5]], [R.KIND_ID["smith_target"]])
        x = torch.full((1, R.N_ACT), float("-inf"))
        x[0, 300], x[0, 301], x[0, 331] = np.log(0.3), np.log(0.3), np.log(0.4)
        x.requires_grad_(True)
        out, _ = kl.kl(torch.log_softmax(x, -1), torch.tensor([[0, -1, -1]]), 0.1)
        self.assertAlmostEqual(out.item(), -kl.cfg.leave * np.log(0.6), places=5)  # KL part 0
        out.sum().backward()
        self.assertGreater(x.grad[0, 331].item(), 0)  # descent lowers the cancel
        rest = self.kl.kl(torch.log_softmax(x.detach(), -1), torch.tensor([[0, -1, -1]]), 0.1)
        self.assertEqual(R.KINDS[0] in R.LEAVE_KINDS, False)
        self.assertTrue(torch.isfinite(rest[0]).all())

    def test_an_option_is_the_sum_of_its_actions(self):
        a2o = torch.full((1, R.N_ACT), -1, dtype=torch.int8)
        a2o[0, [212, 228, 354]] = 3  # skip, its alternative, proceed: one SKIP option
        a2o[0, [172, 173, 174]] = torch.tensor([0, 1, 2], dtype=torch.int8)
        x = torch.full((1, R.N_ACT), float("-inf"))
        x[0, [172, 173, 174, 212, 228, 354, 164]] = 0.0
        q = R.option_logq(torch.log_softmax(x, -1), a2o).exp()[0, :4]
        self.assertTrue(torch.allclose(q, torch.tensor([1 / 6, 1 / 6, 1 / 6, 1 / 2])))

    def test_beta_schedule(self):
        c = R.PriorCfg()
        self.assertEqual(c.beta(0), 0.1)
        self.assertEqual(c.beta(250_000_000), 0.1)
        self.assertAlmostEqual(c.beta(375_000_000), 0.06)
        self.assertAlmostEqual(c.beta(500_000_000), 0.02)
        self.assertAlmostEqual(c.beta(10**9), 0.02)


@unittest.skipUnless(HAVE_PRIOR, "the RLN41 prior is local only")
class Adapter(unittest.TestCase):
    def test_entries_score_as_the_rln41_adapter(self):
        """On a real env (random legal actions): every entry's prior equals `collate` of the
        RLN41 adapter's record (restricted to the legal options), and its chosen option too."""
        from sts2 import E2E, Sts2RunEnvs
        n = 32
        env = Sts2RunEnvs(n, seeds=np.arange(n, dtype=np.uint64) + 11, character="Necrobinder",
                          ascension=10, combat="greedy", threads=2, **E2E)
        model = P.load_model(P.DATA / "prior.pt")
        rng = np.random.default_rng(0)
        seen = set()
        try:
            for _ in range(1500):
                legal = R.legal_bits(env.masks)
                acts = np.array([rng.choice(np.nonzero(m[:R.N_ACT])[0]) if m.any() else 0
                                 for m in legal], np.uint16)
                sc = R.scan(env.obs, env.masks)
                if len(sc):
                    lp = R.prior_logp(model, sc.obs, sc.kind, sc.opts, torch.device("cpu")).numpy()
                    for j in range(len(sc)):
                        i, a = sc.row[j], int(acts[sc.row[j]])
                        ch = sc.a2o[j, a]
                        if ch < 0:
                            continue
                        recs = [r for r in P.obs_records(env.obs[i], a, set())
                                if P.KIND_ID[r["kind"]] == sc.kind[j]]
                        self.assertTrue(recs)
                        mine = [tuple(o) for o in sc.opts[j]]
                        r = recs[0]
                        keep = [k for k, o in enumerate(r["options"]) if tuple(o) in mine]
                        r = dict(r, options=[r["options"][k] for k in keep],
                                 choice=keep.index(r["choice"]))
                        q = dict(zip(map(tuple, r["options"]), P.scores(model, [r])[0]))
                        self.assertEqual(set(q), set(mine))
                        for k, o in enumerate(mine):
                            self.assertAlmostEqual(float(np.exp(lp[j, k])), float(q[o]), places=4)
                        self.assertEqual(mine[ch], tuple(r["options"][r["choice"]]))
                        seen.add(R.KINDS[sc.kind[j]])
                env.step(acts)
        finally:
            env.close()
        self.assertTrue({"card_reward", "rest", "shop_card", "event", "ancient"} <= seen, seen)


@unittest.skipUnless(HAVE_PRIOR, "the RLN41 prior is local only")
class LearnerPrior(unittest.TestCase):
    def test_one_iteration_with_the_prior_logs_it(self):
        d = pathlib.Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, d, True)
        learner = T.Learner(tiny_config(run_head=True, steps=48, prior={}), d / "r")
        self.addCleanup(learner.close)
        rec = learner.iterate()
        self.assertTrue(rec["finite"])
        self.assertIn("prior", rec)
        self.assertIn("prior_beta", rec)
        self.assertEqual(learner.prior.next_id, rec["prior"]["entries"])

    def test_prior_none_is_settable_and_default_off(self):
        self.assertIsNone(T.Config().prior)
        cfg = tiny_config()
        T.set_resumed(cfg, "prior", {"coef": 0.05})
        self.assertEqual(cfg.prior, {"coef": 0.05})
        T.set_resumed(cfg, "prior", None)
        self.assertIsNone(cfg.prior)
        self.assertEqual(dataclasses.asdict(tiny_config())["prior"], None)


class BehaviourTelemetry(unittest.TestCase):
    def run_one(self, **kw):
        d = pathlib.Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, d, True)
        learner = T.Learner(tiny_config(steps=160, **kw), d / "r")
        self.addCleanup(learner.close)
        rec = learner.iterate()
        return learner, rec

    def test_off_and_on_train_the_same(self):
        off, r0 = self.run_one()
        on, r1 = self.run_one(behaviour=True)
        self.assertNotIn("behaviour", r0)
        for a, b in zip(off.net.parameters(), on.net.parameters()):
            self.assertTrue(torch.equal(a, b))
        on.save()  # the overlap's `behaviour` net is another attribute
        b, n = r1["behaviour"], r1["behaviour"]["counts"]
        self.assertEqual(n["runs"], r1["runs"])  # every finished normal run is counted
        self.assertGreater(n["runs"], 0)
        self.assertEqual(sum(n["death_act"]), n["deaths"])
        self.assertAlmostEqual(b["floor"], r1["mean_floor"], places=3)

    def test_counts_rest_skip_removal_potions(self):
        import rln42_watch as W
        beh = W.Behaviour(2)
        D = R.D
        legal = np.zeros(R.N_ACT, bool)
        o = np.zeros((2, 700), np.uint8)
        o[:, 8], o[:, 10] = 80, 100  # HP 80/100
        a2o = np.full((3, R.N_ACT), -1, np.int8)
        a2o[0, 10], a2o[0, 11] = 0, 1  # rest: action 10 heal, 11 smith
        a2o[1, 20], a2o[1, 21] = 0, 1  # card reward: 20 pick, 21 skip
        sc = R.Scan(row=np.array([0, 1, 1]), kind=np.array([R.KINDS.index("rest"),
                                                             R.KINDS.index("card_reward"),
                                                             R.KINDS.index("shop_removal")]),
                    opts=[[(D.T_HEAL, 0, 0), (D.T_SMITH, 0, 0)],
                          [(D.T_CARD, 1, 0), (D.T_SKIP, 0, 0)],
                          [(D.T_REMOVE, 0, 0), (D.T_NONE, 0, 0)]],
                    a2o=a2o, obs=o[[0, 1, 1]])
        pot = int(np.nonzero(beh.potion_use)[0][0])
        beh.step(sc, np.array([11, 21]), np.array([7, 4]))
        beh.step(R.Scan(np.zeros(0, int), np.zeros(0, int), [], a2o[:0], o[:0]),
                 np.array([beh.cancel, pot]), np.array([5, 3]))  # env 0 backs out
        beh.step(sc, np.array([11, 21]), np.array([7, 4]))  # env 0: the same rest visit
        fo = np.zeros((2, 700), np.uint8)
        fo[0, W.BELT_AT], fo[0, W.BELT_AT + 2] = 1, 2  # two potions held
        fo[0, W.GOLD_AT] = 77
        beh.ended(np.array([True, False]), np.array([False, True]), fo, np.array([9, 4]),
                  np.array([0, 0]), np.array([True, False]), np.array([3, 0]))
        rec = beh.record()
        n = rec["counts"]
        self.assertEqual((n["runs"], n["rest"], n["smith_hi"], n["potions"]), (1, 1, 1, 0))
        self.assertEqual((n["cancels"], n["run_rows"]), (1, 3))
        self.assertEqual((n["rewards"], n["removal_visits"]), (0, 0))  # env 1: practice, dropped
        self.assertEqual((rec["smith_hi"], rec["death_2pot"], rec["gold_death"],
                          rec["upgrades_per_run"]), (1.0, 1.0, 77.0, 3.0))
        self.assertEqual(beh.cnt.sum(), 0)
        self.assertEqual(legal.sum(), 0)


class HeldCap(unittest.TestCase):
    def test_held_max_shortens_the_hold(self):
        d = pathlib.Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, d, True)
        base = dict(steps=48, returns="two_chain", adv_norm="per_type")
        free = T.Learner(tiny_config(**base), d / "a")
        self.addCleanup(free.close)
        capped = T.Learner(tiny_config(held_max=5, **base), d / "b")
        self.addCleanup(capped.close)
        r0, r1 = free.iterate(), capped.iterate()
        self.assertEqual(r0["hold_iters"], T.R.HOLD_ITERS)
        self.assertGreater(r0["rows_held"], 5)
        self.assertLess(r1["hold_iters"], T.R.HOLD_ITERS)
        if r1["hold_iters"] > 0:  # at 0 only dormant rows stay, whatever their number
            self.assertLessEqual(r1["rows_held"], 5)


class Watchdog(unittest.TestCase):
    @staticmethod
    def recs(n, smith=60, floor=lambda d: 5 + d / 1e7):
        out = []
        for k in range(1, n + 1):
            d = k * 1_000_000
            out.append(dict(decisions=d, runs=100, mean_floor=floor(d), behaviour=dict(counts=dict(
                runs=100, rest=100, smith=smith, rest_hi=100, smith_hi=smith, rewards=100,
                skips=20, removals=30, removal_visits=50, potions=80, upgrades=0, deaths=100,
                deaths_2pot=0, gold_death_sum=0, floor_death_sum=0, floor_sum=0, wins=0,
                death_act=[100, 0, 0]))))
        return out

    def test_pass_fail_and_floor_streak(self):
        import rln42_watch as W
        cfg = W.DEFAULTS
        self.assertEqual(W.check(self.recs(130), 125_000_000, cfg)[0], [])
        fails = W.check(self.recs(30, smith=40), 25_000_000, cfg)[0]
        self.assertEqual([f[0] for f in fails], ["smith_hi"])
        flat = self.recs(130, floor=lambda d: 20.0)
        self.assertEqual([f[0] for f in W.check(flat, 125_000_000, cfg)[0]], ["floor_rise"])
        once = self.recs(130, floor=lambda d: 20.0 if d > 95_000_000 else 5 + d / 1e7)
        self.assertEqual(W.check(once, 100_000_000, cfg)[0], [])  # one flat check is noise

    def test_watch_pauses_and_writes_the_alert(self):
        import json
        import rln42_watch as W
        d = pathlib.Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, d, True)
        (d / "stats.jsonl").write_text("".join(json.dumps(r) + "\n" for r in self.recs(55, smith=40)))
        W.watch_once(d)
        self.assertTrue((d / "PAUSE").exists())
        alert = json.loads((d / "WATCH_ALERT").read_text())
        self.assertEqual((alert["decisions"], alert["failures"][0]["metric"]), (25_000_000, "smith_hi"))
        self.assertTrue((d / "watch.json").exists())
        log = (d / "watch.log").read_text().splitlines()
        self.assertEqual(len(log), 1)  # stops at the alert
        W.watch_once(d)
        self.assertEqual(len((d / "watch.log").read_text().splitlines()), 1)


if __name__ == "__main__":
    unittest.main()
