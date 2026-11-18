"""NAZ1: the fight-outcome model's consistency and the start-record round trip.
`python -m unittest python/tests/test_naz1.py` (after `cargo build --profile ffi -p sts2-ffi`)."""

import json
import pathlib
import sys
import unittest

import numpy as np
import torch

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

import naz1_model as M  # noqa: E402

REC = dict(act=1, act_id="HIVE", encounter="KAISER_CRAB_BOSS", floor=34, hp=41, max_hp=77, gold=123,
           deck=[["STRIKE_NECROBINDER", 0], ["STRIKE_NECROBINDER", 1], ["DEFEND_NECROBINDER", 0],
                 ["BODYGUARD", 1], ["UNLEASH", 0]],
           relics=["BOUND_PHYLACTERY"], potion_slots=2, potions=["BLOCK_POTION"])


class FightModelTest(unittest.TestCase):
    def test_outcomes_are_consistent(self):
        vocab = M.Vocab.build([REC])
        net = M.FightNet({k: vocab.size(k) for k in M.Vocab.KINDS}, d=32, layers=1, heads=2).eval()
        low = dict(REC, hp=5)
        tok = M.to_dev(M.tokenize(vocab, [REC, low, dict(REC, deck=REC["deck"] + [["NOPE", 0]])]),
                       torch.device("cpu"))
        with torch.no_grad():
            logp = net(tok)
            pd, mean, p = net.summary(logp, tok["hp"])
        self.assertTrue(torch.allclose(p.sum(-1), torch.ones(3), atol=1e-5))
        # Surviving with a loss >= the entry HP has probability 0.
        values = M.LOSS_VALUES
        self.assertEqual(float(p[1, :M.NB_LOSS][values >= 5].sum()), 0.0)
        manual = float((p[1, :M.NB_LOSS] * values).sum() + pd[1] * 5)
        self.assertAlmostEqual(float(mean[1]), manual, places=4)
        self.assertEqual(int(tok["cards"][2, :, 0].eq(1).sum()), 1)  # the unknown card is <unk>

    def test_targets_skip_bad_rollouts(self):
        lab = dict(hp_lost=np.array([[3, 50, 41]]), died=np.array([[False, False, True]]),
                   ok=np.array([[True, False, True]]))
        hist, mean = M.targets([REC], lab)
        self.assertAlmostEqual(float(hist.sum()), 1.0)
        self.assertAlmostEqual(float(hist[0, M.NB_LOSS]), 0.5)
        self.assertAlmostEqual(float(mean[0]), (3 + 41) / 2)


class StartRecordTest(unittest.TestCase):
    def test_gym_fight_round_trips(self):
        from sts2 import ffi
        from sts2.env import Sts2RunEnvs

        lib = ffi.load()
        if not hasattr(lib, "sts2_run_envs_start_record"):
            self.skipTest("library without sts2_run_envs_start_record")
        path = pathlib.Path(__file__).resolve().parents[2] / "target" / "naz1-test-gym.jsonl"
        path.write_text(json.dumps(REC) + "\n")
        env = Sts2RunEnvs(1, character="Necrobinder", ascension=10, seed_range=(0, 1), cycle=True)
        try:
            env.load_gym(path)
            env.set_gym(first=0, slots=1, sequential=True, seed=5)
            env.reset_all()
            got = json.loads(env.start_record(0))
            for k in ("act", "act_id", "encounter", "floor", "hp", "max_hp", "deck", "relics",
                      "potions"):
                self.assertEqual(got[k], REC[k], k)
        finally:
            env.close()
            path.unlink(missing_ok=True)


if __name__ == "__main__":
    unittest.main()
