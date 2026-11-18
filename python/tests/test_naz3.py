"""NAZ3: the search player's copies leave the real fight alone and it plays legal moves.
`python -m unittest python/tests/test_naz3.py` (after `cargo build --profile ffi -p sts2-ffi`).
The net is stubbed (uniform over legal actions, with a small bias so the top-k is defined)."""

import json
import pathlib
import sys
import unittest

import numpy as np
import torch

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

import naz3_combat as N  # noqa: E402

REC = dict(act=0, act_id="OVERGROWTH", encounter="FUZZY_WURM_CRAWLER_WEAK", floor=2, hp=52,
           max_hp=66, gold=99,
           deck=[["STRIKE_NECROBINDER", 0]] * 4 + [["DEFEND_NECROBINDER", 0]] * 4
           + [["BODYGUARD", 0], ["UNLEASH", 0]],
           relics=["BOUND_PHYLACTERY"], potion_slots=2, potions=["BLOCK_POTION"])


def stub_logits(net, env, idx, dev):
    from sts2.e2e import encode as E

    if not len(idx):
        return None
    legal = E.mask_bits(torch.from_numpy(env.masks.view(np.int64)[np.asarray(idx)]))
    bias = -torch.arange(legal.shape[1], dtype=torch.float32) * 1e-3
    return bias.expand(legal.shape).masked_fill(~legal, float("-inf")).clone()


class SearchPlayerTest(unittest.TestCase):
    def setUp(self):
        from sts2 import ffi

        if not ffi.has_gym(ffi.load()):
            self.skipTest("library without the gym")
        self.saved = N.logits
        N.logits = stub_logits

    def tearDown(self):
        N.logits = self.saved

    def test_search_keeps_the_fight_and_plays_legal_moves(self):
        from sts2 import E2E, HOLD
        from sts2.env import Sts2RunEnvs

        path = pathlib.Path(__file__).resolve().parents[2] / "target" / "naz3-test-gym.jsonl"
        path.write_text(json.dumps(REC) + "\n")
        player = N.SearchPlayer(k=2, m=3, pmin=0.0, pskip=1.0, loss_w=0.5, seed=1)
        player.base = 1
        env = Sts2RunEnvs(1 + player.scratch(1), character="Necrobinder", ascension=10,
                          seed_range=(0, 1), cycle=True, combat="agent", obs_v2=True, **E2E)
        try:
            self.assertEqual(env.load_gym(path), 1)
            env.set_gym(first=0, slots=1, sequential=True, seed=5)
            env.reset_all()
            self.assertEqual(json.loads(env.start_record(0))["encounter"], REC["encounter"])
            self.assertAlmostEqual(float(N.enemy_dealt(env.obs[:1])[0]), 0.0)
            dev = torch.device("cpu")
            for _ in range(12):
                if env.done[0]:
                    break
                before = env.obs[0].copy()
                lg = N.logits(None, env, np.array([0]), dev)
                a = player.choose(None, env, dev, np.array([0]), lg, np.array([True]))
                self.assertTrue(np.array_equal(env.obs[0], before), "search moved the real fight")
                self.assertTrue(np.isfinite(float(lg[0, int(a[0])])), "illegal move")
                acts = np.full(env.n, HOLD, dtype=np.uint16)
                acts[0] = a[0]
                env.step(acts)
            self.assertGreater(player.stats["searched"], 0)
            self.assertEqual(player.stats["capped"], 0)
        finally:
            env.close()
            path.unlink(missing_ok=True)


if __name__ == "__main__":
    unittest.main()
