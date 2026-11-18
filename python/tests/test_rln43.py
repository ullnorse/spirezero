"""RLN43 combat gym (`practice.human`, `sts2_rl::human`):

* the gym's cell weights and slot split (bosses, hallways, acts 2-3 weighted 2x; share 1.0 is
  combat-only);
* off is bit-identical: no `human`, `human.enabled=False`, and a gym on with share 0 (loaded,
  no slots) train to the same weights and rewards;
* on: gym envs play terminal fights (`gym_end`, never bootstrapped), their rows get no run
  reward and the cell's `win_bonus` once per survived fight; it trains and resumes;
* combat-only (`practice.share=1.0`): every env plays the gym;
* the env binding: sequential starts in record order, cell codes 7-15;
* `explore_floor` / `kind_log`: the per-kind option stats and KL, both KL directions' gradient at
  a collapsed option, the floor switching on and off, off (and logging) bit-identical, and a
  resume that turns the log on over held rows.

    python -m unittest python/tests/test_rln43.py   (after the ffi build)
"""

import json
import pathlib
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

import numpy as np  # noqa: E402
import torch  # noqa: E402

import train_e2e as T  # noqa: E402
from sts2 import E2E, HOLD, Sts2RunEnvs  # noqa: E402
from sts2.env import GYM_END, RUN_ENDS  # noqa: E402
from sts2.ffi import GYM_CODE, cell_name  # noqa: E402
from test_e2e_learner import tiny_config  # noqa: E402
from sts2.e2e.encode import REST_AT as E_REST  # noqa: E402

SCRATCH = pathlib.Path(__file__).resolve().parents[2] / "target"
SCRATCH.mkdir(exist_ok=True)

DECK = [["STRIKE_NECROBINDER", 0]] * 3 + [["DEFEND_NECROBINDER", 0]] * 3 + [["BODYGUARD", 1]]


def record(act, act_id, enc, floor, hp=30):
    return dict(act=act, act_id=act_id, encounter=enc, floor=floor, hp=hp, max_hp=70, gold=99,
                deck=DECK, relics=["BOUND_PHYLACTERY"], potion_slots=2, potions=["BLOCK_POTION"])


RECORDS = [record(0, "OVERGROWTH", "BYRDONIS_ELITE", 9), record(0, "UNDERDOCKS", "SOUL_FYSH_BOSS", 17),
           record(1, "HIVE", "KAISER_CRAB_BOSS", 34), record(1, "HIVE", "ENTOMANCER_ELITE", 25),
           record(2, "GLORY", "QUEEN_BOSS", 51), record(2, "GLORY", "KNIGHTS_ELITE", 40),
           record(1, "HIVE", "THE_INSATIABLE_BOSS", 34, hp=5)]


def write_gym(path):
    path.write_text("".join(json.dumps(r) + "\n" for r in RECORDS))
    return str(path)


def weights(a):
    return [p.detach().clone() for p in a.net.parameters()]


class Settings(unittest.TestCase):
    def test_weights_and_split(self):
        h = T.Human(enabled=True)
        w = np.array(h.weights(), dtype=float) / 1e6
        self.assertAlmostEqual(w[3:6].sum(), 0.65, places=5)
        self.assertAlmostEqual(w[6:9].sum(), 0.12, places=5)
        self.assertAlmostEqual(w[6], 0.0)
        self.assertAlmostEqual(w[4] / w[3], 2.0, places=4)
        self.assertAlmostEqual(w[2] / w[0], 2.0, places=4)
        self.assertEqual(h.split(1024, 2048), (512, 512))
        self.assertEqual(h.split(2048, 2048), (0, 2048))

    def test_resume_merge_and_old_configs(self):
        cfg = tiny_config()
        self.assertIsNone(cfg.practice.gym())
        T.set_resumed(cfg, "practice", {"enabled": True, "human": {"enabled": True, "hp_coef": 0.05}})
        T.set_resumed(cfg, "practice", {"human": {"boss": 0.7}})
        g = cfg.practice.gym()
        self.assertEqual((g.hp_coef, g.boss, g.share), (0.05, 0.7, 0.5))
        back = T.Config.from_dict(T.dataclasses.asdict(cfg))
        self.assertEqual(back.practice, cfg.practice)
        self.assertEqual(T.Human(win_bonus=0.05).bonus().tolist(), [0.05] * 9)
        self.assertEqual(T.Human.from_dict(dict(win_bonus=list(range(9)))).bonus()[4], 4.0)
        with self.assertRaises(ValueError):
            T.Human.from_dict(dict(win_bonus=[0.1, 0.2]))
        d = T.dataclasses.asdict(T.Config())
        del d["practice"]["human"]
        self.assertIsNone(T.Config.from_dict(d).practice.gym())


class Binding(unittest.TestCase):
    def test_sequential_starts_and_codes(self):
        tmp = pathlib.Path(tempfile.mkdtemp(dir=SCRATCH))
        try:
            env = Sts2RunEnvs(10, seed_range=(5, 50), ascension=10, threads=1, **E2E)
            self.assertEqual(env.load_gym(write_gym(tmp / "g.jsonl")), len(RECORDS))
            env.set_gym(first=2, slots=7, sequential=True, seed=11)
            env.reset_all()
            names = [cell_name(int(c)) for c in env.cell]
            self.assertEqual(names[:2], ["RUN", "RUN"])
            self.assertEqual(names[2:9], ["GYM_ELITE_ACT1", "GYM_BOSS_ACT1", "GYM_BOSS_ACT2",
                                          "GYM_ELITE_ACT2", "GYM_BOSS_ACT3", "GYM_ELITE_ACT3",
                                          "GYM_BOSS_ACT2"])
            self.assertEqual(names[9], "RUN")
            self.assertEqual(env.gym_stats()["started"][4], 2)
            # The gym's HP is the record's: the combat section's live HP.
            hp = T.hp_fraction(env.obs[2:9])
            np.testing.assert_allclose(hp, [r["hp"] / 70 for r in RECORDS])
            env.close()
        finally:
            shutil.rmtree(tmp)


class Learner(unittest.TestCase):
    def setUp(self):
        self.dir = pathlib.Path(tempfile.mkdtemp(dir=SCRATCH))
        self.gym = write_gym(self.dir / "gym.jsonl")

    def tearDown(self):
        shutil.rmtree(self.dir)

    def practice(self, human=None, share=(0.5, 0.5)):
        p = dict(enabled=True, min_active=1, max_uses=0, share=list(share))
        if human is not None:
            p["human"] = dict(path=self.gym, **human)
        return p

    def train(self, name, practice, iters=2, steps=96, **kw):
        a = T.Learner(tiny_config(steps=steps, practice=practice, **kw), self.dir / name)
        recs = [a.iterate() for _ in range(iters)]
        a.save(self.dir / name / "state.pt")
        w = weights(a)
        a.close()
        return w, recs

    def test_off_is_bit_identical(self):
        base, rb = self.train("base", self.practice())
        for name, human in (("disabled", dict(enabled=False)),
                            ("share0", dict(enabled=True, share=0.0, hp_coef=0.1))):
            w, r = self.train(name, self.practice(human))
            self.assertTrue(all(torch.equal(p, q) for p, q in zip(base, w)), name)
            self.assertEqual([x["reward_sum"] for x in rb], [x["reward_sum"] for x in r], name)

    def test_gym_fights_are_terminal_and_rewarded_once(self):
        a = T.Learner(tiny_config(steps=400, practice=self.practice(
            dict(enabled=True, share=1.0, hall=0.0, hp_coef=0.0))), self.dir / "on")
        self.assertEqual(a.envs.cell[:4].tolist(), [0] * 4)  # own slots: 0 to 2, gym 2 to 4
        buf = a._alloc()
        envs = a.envs
        rec = []
        step0 = envs.step

        def step(actions):
            c = envs.cell.copy()
            step0(actions)
            rec.append((c, envs.done.astype(bool).copy(), envs.end.copy(),
                        envs.ended_cell.copy()))
        envs.step = step
        runs = a._collect_into(buf, a.net, a.gen)
        envs.step = step0
        reward = buf["reward"].numpy().astype(np.float64)
        gym = buf["gym"].numpy()
        cells = np.stack([r[0] for r in rec])
        self.assertTrue(np.array_equal(gym, cells >= GYM_CODE))
        self.assertTrue(gym.any())
        ends = [(t, e) for t, (c, d, end, ec) in enumerate(rec) for e in np.nonzero(d & (ec >= GYM_CODE))[0]]
        self.assertTrue(ends, "a gym fight ended")
        bonus = a.cfg.practice.gym().bonus()
        for t, e in ends:
            end = RUN_ENDS[rec[t][2][e]]
            self.assertIn(end, ("gym_end", "died", "victory", "fight_capped", "decision_capped"))
            want = bonus[rec[t][3][e] - GYM_CODE] if end in ("gym_end", "victory") else 0.0
            self.assertAlmostEqual(reward[t, e], want, places=6)
        # Every other gym row: no reward at all (no floors, bosses or run win).
        mask = gym.copy()
        for t, e in ends:
            mask[t, e] = False
        self.assertTrue(np.all(reward[mask] == 0))
        # Terminal: never a practice truncation, so nothing bootstraps.
        self.assertFalse(buf["boot"].numpy()[gym].any())
        if "trunc" in buf:
            self.assertFalse(buf["trunc"].numpy()[gym].any())
        self.assertNotIn(T.PRACTICE_END, [rec[t][2][e] for t, e in ends])
        self.assertIn("gym", runs)
        a.close()

    def test_gym_deaths_skip_adaptive(self):
        # Combat-only gym with one-HP starts: every death is a gym death; RLN37's D stays 0.
        a = T.Learner(tiny_config(steps=200, practice=self.practice(
            dict(enabled=True), share=(1.0, 1.0)) | dict(adaptive=True)), self.dir / "ad")
        a.envs.reset_all()
        rec = a.iterate()
        self.assertGreater(sum(rec["gym"]["started"].values()), 0)
        self.assertEqual(sum(a.adapt_deaths), 0)
        a.close()

    def test_trains_resumes_and_combat_only(self):
        w, recs = self.train("on", self.practice(dict(enabled=True, hp_coef=0.05)), iters=3)
        self.assertTrue(all(r["finite"] for r in recs))
        self.assertIn("gym", recs[-1])
        self.assertIn("gym_mass", recs[-1])
        m = recs[-1]["gym_mass"]
        self.assertAlmostEqual(m["mass_gym"], 0.6 * m["rows_gym"] / (1 - 0.4 * m["rows_gym"]),
                               places=5)
        cfg = tiny_config(steps=96, practice=self.practice(dict(enabled=True, hp_coef=0.05)))
        b = T.Learner(cfg, self.dir / "on", resume=True)
        self.assertIsNotNone(b.cfg.practice.gym())
        self.assertTrue(b.iterate()["finite"])
        b.close()
        # Combat-only: every env plays the gym once its first episode ends.
        a = T.Learner(tiny_config(steps=64, practice=self.practice(
            dict(enabled=True), share=(1.0, 1.0))), self.dir / "only")
        a.envs.reset_all()
        self.assertTrue((a.envs.cell >= GYM_CODE).all())
        self.assertTrue(a.iterate()["finite"])
        a.close()


REST0 = T.REST_ACTIONS.start


def rest_logits(q_smith, legal=2, width=355):
    """One rest-site row: `legal` rest options, Smith (slot 1) with probability `q_smith`."""
    lg = torch.full((1, width), float("-inf"))
    lg[0, REST0:REST0 + legal] = 0.0
    lg[0, REST0 + 1] = float(np.log(q_smith / (1 - q_smith)))
    return lg


class Kinds(unittest.TestCase):
    def setUp(self):
        self.kt = T.KindTables("cpu")

    def test_rest_stats_and_directions(self):
        run = torch.tensor([True])
        for direction, big in (("pi_u", False), ("u_pi", True)):
            lg = rest_logits(2e-4).requires_grad_()
            on, lq, legal, n, frac = self.kt.stats("rest", torch.log_softmax(lg, -1), run)
            self.assertTrue(bool(on[0]))
            self.assertEqual(int(n[0]), 2)
            self.assertAlmostEqual(float(lq[0, 1].detach().exp()), 2e-4, places=6)
            kl = T.explore_kl(lq, legal, n, on, direction)
            kl.sum().backward()
            g = float(lg.grad[0, REST0 + 1])  # d KL / d Smith logit (descent raises it if < 0)
            self.assertLess(g, 0)
            # pi_u: q (log q - log(1 - q)) ~ -1.7e-3; u_pi: q - 1/2 ~ -0.5.
            self.assertAlmostEqual(g, -0.4998 if big else 2e-4 * np.log(2e-4 / (1 - 2e-4)),
                                   places=3)
        # Not a rest row in a fight, or with one legal option.
        on = self.kt.stats("rest", torch.log_softmax(rest_logits(0.5), -1), torch.tensor([False]))[0]
        self.assertFalse(bool(on[0]))
        lg = torch.full((1, 355), float("-inf"))
        lg[0, REST0] = 0.0
        self.assertFalse(bool(self.kt.stats("rest", torch.log_softmax(lg, -1), run)[0][0]))

    def test_remainder_option(self):
        # A shop row: a removal and 3 other legal actions: removal yes/no is 2 options.
        from sts2.env import ENTITY_KINDS, action_table
        ek = action_table()[:355, 0]
        rem = int(np.flatnonzero(ek == ENTITY_KINDS.index("shop_remove"))[0])
        other = np.flatnonzero(ek == ENTITY_KINDS.index("shop_card"))[:3]
        lg = torch.full((1, 355), float("-inf"))
        lg[0, rem] = 0.0
        lg[0, other] = 0.0
        on, lq, legal, n, frac = self.kt.stats("removal", torch.log_softmax(lg, -1),
                                               torch.tensor([True]))
        self.assertEqual(int(n[0]), 2)
        self.assertAlmostEqual(float(lq[0].exp()[legal[0]].min()), 0.25, places=5)
        self.assertAlmostEqual(float(frac[0]), float(-(0.25 * np.log(.25) + .75 * np.log(.75))
                                                     / np.log(2)), places=5)
        p = self.kt.named_p("removal", torch.log_softmax(lg, -1), on)["remove"]
        self.assertAlmostEqual(float(p[0] / p[1]), 0.25, places=5)

    def test_switching(self):
        ex = T.ExploreFloor(floors={"rest": 0.15, "card_reward": 0.15}, coef=0.05,
                            decay_steps=100, start=10)
        self.assertEqual(ex.active({}), set())  # not measured yet: off
        self.assertEqual(ex.active({"rest": 0.01, "card_reward": 0.5}), {"rest"})
        self.assertEqual(ex.active({"rest": 0.2, "card_reward": 0.1}), {"card_reward"})
        self.assertEqual(ex.active({"rest": None, "card_reward": 0.15}), set())
        self.assertEqual((ex.coef_at(10), ex.coef_at(60), ex.coef_at(500)), (0.05, 0.025, 0.0))
        with self.assertRaises(ValueError):
            T.ExploreFloor(floors={"smith": 0.1})
        with self.assertRaises(ValueError):
            T.ExploreFloor(direction="up")

    def test_rest_rows(self):
        o = np.zeros((3, 3000), np.uint8)
        o[:2, 3] = 7
        o[:, 8], o[:, 10] = 30, 100  # header HP 30 / 100
        k = o[:, E_REST:E_REST + 16]
        k[0, :4] = [1, 1, 2, 1]  # heal, smith (enabled)
        k[1, :4] = [1, 1, 2, 0]  # smith disabled
        o[:, E_REST:E_REST + 16] = k
        slot, band = T.rest_rows(o)
        self.assertEqual(slot.tolist(), [1, -1, -2])
        self.assertEqual(band.tolist(), [1, 1, 1])


class Explore(unittest.TestCase):
    def setUp(self):
        self.dir = pathlib.Path(tempfile.mkdtemp(dir=SCRATCH))

    def tearDown(self):
        shutil.rmtree(self.dir)

    def train(self, name, iters=3, **kw):
        a = T.Learner(tiny_config(steps=64, envs=16, **kw), self.dir / name)
        out = []
        for _ in range(iters):
            r = a.iterate()
            out.append((weights(a), r))
        a.close()
        return out

    def same(self, x, y):
        return all(torch.equal(p, q) for p, q in zip(x, y))

    def test_off_and_logging_are_bit_identical(self):
        base = self.train("base", kind_log=False)
        for name, kw in (("log", dict(kind_log=True)),
                         ("floor0", dict(explore_floor=dict(floors={"map": 0.0, "rest": 0.0}))),
                         ("coef0", dict(explore_floor=dict(floors={"map": 1.0}, coef=0.0)))):
            got = self.train(name, **kw)
            for (wa, ra), (wb, rb) in zip(base, got):
                self.assertTrue(self.same(wa, wb), name)
                self.assertEqual(ra["reward_sum"], rb["reward_sum"], name)
            self.assertIn("explore", got[-1][1])
        self.assertNotIn("explore", base[-1][1])
        k = got[-1][1]["explore"]["kinds"]
        self.assertGreater(k["map"]["rows"], 0)
        self.assertTrue(0 < k["map"]["ent"] <= 1)

    def test_floor_switches_on_below_and_off_above(self):
        base = self.train("base", kind_log=True)
        # map's entropy fraction ~1: a floor of 1.01 is above it (on from the 2nd iteration,
        # after the 1st measurement); 0.5 is below it (never on).
        on = self.train("on", explore_floor=dict(floors={"map": 1.01}, coef=0.05))
        off = self.train("off", explore_floor=dict(floors={"map": 0.5}, coef=0.05))
        self.assertEqual([r["explore"]["on"] for _, r in on], [[], ["map"], ["map"]])
        self.assertEqual([r["explore"]["on"] for _, r in off], [[], [], []])
        self.assertTrue(self.same(base[0][0], on[0][0]))
        self.assertFalse(self.same(base[1][0], on[1][0]))
        self.assertTrue(all(self.same(a[0], b[0]) for a, b in zip(base, off)))
        self.assertTrue(all(r["finite"] for _, r in on))

    def test_resume_turns_logging_on_over_held_rows(self):
        cfg = dict(steps=64, envs=16, kind_log=False, returns="two_chain")
        a = T.Learner(tiny_config(**cfg), self.dir / "a")
        a.iterate()
        a.iterate()
        a.save(self.dir / "a" / "state.pt")
        self.assertTrue(a.held)
        a.close()
        b = T.Learner(tiny_config(**{**cfg, "kind_log": True,
                                     "explore_floor": dict(floors={"rest": 0.15})}),
                      self.dir / "a", resume=True)
        self.assertEqual(b.cfg.explore_floor["start"], b.decisions)
        r = b.iterate()
        self.assertTrue(r["finite"])
        self.assertIn("explore", r)
        b.close()


class Watch(unittest.TestCase):
    def test_entropy_reported_not_paused(self):
        import rln42_watch as W
        d = pathlib.Path(tempfile.mkdtemp(dir=SCRATCH))
        self.addCleanup(shutil.rmtree, d, True)
        (d / "watch.json").write_text(json.dumps(dict(
            checks={}, floor=dict(start=10**18, lag=1, min_rise=0.0, streak=1))))
        kinds = lambda e: dict(rest=dict(ent=e, rows=150), card_reward=dict(ent=0.5, rows=900),  # noqa: E731
                               removal=dict(ent=None, rows=0))
        recs = [dict(decisions=k * 5_000_000, runs=10, mean_floor=20.0,
                     explore=dict(kinds=kinds(0.003))) for k in range(1, 11)]
        (d / "stats.jsonl").write_text("".join(json.dumps(r) + "\n" for r in recs))
        W.watch_once(d)
        self.assertFalse((d / "PAUSE").exists())
        self.assertFalse((d / "WATCH_ALERT").exists())
        log = (d / "watch.log").read_text().splitlines()
        notes = [x for x in log if x.startswith("note ")]
        self.assertEqual([x.split()[1] for x in notes], ["25000000", "50000000"])
        self.assertIn('"rest": [0.003, 750]', notes[0])
        self.assertNotIn("card_reward", notes[0])
        self.assertIn("WATCH NOTE at 25M", (d / "lane.log").read_text())
        # Before `from` (10M) nothing is reported; too few rows neither.
        self.assertEqual(W.entropy_low(recs, 5_000_000, 5_000_000, W.DEFAULTS["entropy"]), {})
        self.assertEqual(W.entropy_low(recs, 10_000_000, 5_000_000, W.DEFAULTS["entropy"]), {})


class PinBench(unittest.TestCase):
    def test_pin_bench_unit(self):
        import types
        from unittest import mock
        import e2e_lane as L
        lane = types.SimpleNamespace(out=pathlib.Path("/x/NECRO5"), args=None)
        with mock.patch.object(L.subprocess, "run") as run, mock.patch.object(L, "log") as log:
            run.return_value = types.SimpleNamespace(returncode=0)
            L.Lane.start_bench(lane, pathlib.Path("/x/NECRO5/pinned/state-500M.pt"), "500M")
        cmd = run.call_args[0][0]
        self.assertEqual(cmd[:4], ["systemd-run", "--user", "--unit=necro5-bench-500M", "--collect"])
        self.assertIn("CUDA_VISIBLE_DEVICES=", cmd)
        self.assertIn('--out "/x/NECRO5/pinned/gym-500M.json"', cmd[-1])
        self.assertIn("started necro5-bench-500M", log.call_args[0][1])


if __name__ == "__main__":
    unittest.main()
