"""RLN29: the content stats (`sts2_rl::content_stats`, `card_stats`) and the fresh-run options
(`emb_std`, `v_warm`): `python -m unittest python/tests/test_e2e_card_stats.py` (after
`cargo build --profile ffi -p sts2-ffi`).

The stats rows match the engine's numbers for known cards; a token's upgrade flag comes from its
own column; off, the net is the earlier one (same parameters, same init); on, a zero projection
gives the off net's logits to the bit; the learner's `v_warm` updates move the value module
only, and `emb_std` sets the input embeddings' init.
"""

import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import torch  # noqa: E402

import train_e2e as T  # noqa: E402
from sts2 import ffi  # noqa: E402
from sts2.e2e import encode as E  # noqa: E402
from sts2.e2e.net import E2ENet  # noqa: E402

from test_e2e_learner import smoke_states, tiny_config  # noqa: E402

VOCAB = pathlib.Path(__file__).resolve().parents[2] / "crates/sts2-rl/vocab"


def card_row(name):
    return E._OFF["cards"] + (VOCAB / "cards.txt").read_text().splitlines().index(name)


class TestStatsTable(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.table, cols = E.stats_table(columns=True)
        cls.col = {c: i for i, c in enumerate(cols)}

    def get(self, card, name, up=0):
        return float(self.table[up, card_row(card), self.col[name]])

    def test_known_cards_match_the_engine(self):
        s = lambda x: float(E.symlog(torch.tensor(float(x))))  # noqa: E731
        # Strike: 6 damage (9 upgraded), cost 1, an attack on one enemy.
        self.assertEqual(self.get("STRIKE_NECROBINDER", "Damage"), s(6))
        self.assertEqual(self.get("STRIKE_NECROBINDER", "Damage", 1), s(9))
        self.assertEqual(self.get("STRIKE_NECROBINDER", "d_Damage"), s(3))
        self.assertEqual(self.get("STRIKE_NECROBINDER", "cost"), s(1))
        self.assertEqual(self.get("STRIKE_NECROBINDER", "attack"), 1)
        self.assertEqual(self.get("STRIKE_NECROBINDER", "target_enemy"), 1)
        self.assertEqual(self.get("STRIKE_NECROBINDER", "upgraded", 1), 1)
        # Defend: 5 block (8).
        self.assertEqual(self.get("DEFEND_NECROBINDER", "Block"), s(5))
        self.assertEqual(self.get("DEFEND_NECROBINDER", "Block", 1), s(8))
        # Bodyguard: summon 5 (7); Reap: 27 damage (33), cost 3, retain.
        self.assertEqual(self.get("BODYGUARD", "Summon"), s(5))
        self.assertEqual(self.get("BODYGUARD", "Summon", 1), s(7))
        self.assertEqual(self.get("REAP", "Damage", 1), s(33))
        self.assertEqual(self.get("REAP", "cost"), s(3))
        self.assertEqual(self.get("REAP", "retain"), 1)
        # Zap: the upgrade lowers the cost (a negative delta).
        self.assertEqual(self.get("ZAP", "d_cost"), s(-1))
        # Pad and unknown rows are zero; monsters' rows too.
        self.assertTrue((self.table[:, E._OFF["cards"]:E._OFF["cards"] + 2] == 0).all())
        self.assertTrue((self.table[:, E._OFF["monsters"]:E._OFF["powers"]] == 0).all())
        # Relics and potions have rows, the same at both levels.
        rel = self.table[:, E._OFF["relics"]:E._OFF["monsters"], self.col["is_relic"]]
        pot = self.table[:, E._OFF["potions"]:E._OFF["encounters"], self.col["is_potion"]]
        self.assertGreater(int(rel[0].sum()), 100)
        self.assertGreater(int(pot[0].sum()), 40)
        self.assertTrue(torch.equal(self.table[0, E._OFF["relics"]:E._OFF["encounters"]],
                                    self.table[1, E._OFF["relics"]:E._OFF["encounters"]]))

    def test_the_ffi_table_is_the_engines(self):
        cols, t = ffi.content_stats(ffi.load())
        names = [c for c, _ in cols]
        i = (VOCAB / "cards.txt").read_text().splitlines().index("STRIKE_NECROBINDER")
        self.assertEqual(int(t["cards"][i, names.index("Damage")]), 6)
        self.assertEqual(int(t["cards_up"][i, names.index("Damage")]), 9)


class TestNet(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        obs, masks, priv = smoke_states(n=200, priv=True)
        cls.tok = E.encode(obs, masks)
        cls.ptok = E.encode_priv(priv)
        cls.obs = obs

    def test_token_up_reads_the_cards_upgrade(self):
        up = E.token_up(self.tok["kind"], self.tok["num"])
        hand = self.tok["kind"] == E.K["hand"]
        self.assertTrue(torch.equal(up[hand], (self.tok["num"][..., 0][hand] > 0.5).long()))
        other = (self.tok["kind"] == E.K["creature"]) | (self.tok["kind"] == E.K["relic"])
        self.assertEqual(int(up[other].sum()), 0)

    def test_off_is_the_earlier_net_and_a_zero_map_changes_nothing(self):
        torch.manual_seed(0)
        old = E2ENet(d=32, layers=1)
        torch.manual_seed(0)
        off = E2ENet(d=32, layers=1, card_stats=False, emb_std=None)
        self.assertEqual(old.state_dict().keys(), off.state_dict().keys())
        for k, v in old.state_dict().items():
            self.assertTrue(torch.equal(v, off.state_dict()[k]), k)
        torch.manual_seed(0)
        on = E2ENet(d=32, layers=1, card_stats=True)
        # The same init for every earlier parameter (the module is registered last).
        for k, v in old.state_dict().items():
            self.assertTrue(torch.equal(v, on.state_dict()[k]), k)
        for p in old.parameters():  # non-zero heads, so the logits differ by state
            torch.nn.init.normal_(p, std=0.05)
        on.load_state_dict({**on.state_dict(), **old.state_dict()})
        with torch.no_grad():
            torch.nn.init.zeros_(on.cstats.proj.weight)
            a = old(self.tok, self.ptok)
            b = on(self.tok, self.ptok)
            self.assertTrue(torch.equal(a[0], b[0]) and torch.equal(a[1], b[1]))
            torch.nn.init.normal_(on.cstats.proj.weight, std=0.1)
            c = on(self.tok, self.ptok)
        self.assertFalse(torch.equal(a[0], c[0]))
        self.assertTrue(torch.isfinite(c[1]).all())

    def test_emb_std(self):
        torch.manual_seed(0)
        net = E2ENet(d=64, layers=1, emb_std=0.4)
        self.assertAlmostEqual(float(net.ids.weight.detach().std()), 0.4, delta=0.02)
        self.assertAlmostEqual(float(net.kind.weight.detach().std()), 0.4, delta=0.1)
        self.assertAlmostEqual(float(net.akind.weight.detach().std()), 0.02, delta=0.01)


class TestLearner(unittest.TestCase):
    def test_v_warm_moves_the_value_module_only(self):
        with tempfile.TemporaryDirectory() as out:
            cfg = tiny_config(card_stats=True, emb_std=0.4, v_warm=1, obs_v2=True)
            ln = T.Learner(cfg, out)
            value = {id(p) for p in ln.net.value_parameters()}
            before = {n: p.detach().clone() for n, p in ln.net.named_parameters()}
            rec = ln.iterate()
            self.assertTrue(rec["finite"] and rec.get("v_only"))
            self.assertEqual(rec["lr_factor"], 1.0)
            moved = {n for n, p in ln.net.named_parameters() if not torch.equal(p, before[n])}
            names = {n for n, p in ln.net.named_parameters() if id(p) in value}
            self.assertTrue(moved, "nothing moved")
            self.assertLessEqual(moved, names)
            before = {n: p.detach().clone() for n, p in ln.net.named_parameters()}
            rec = ln.iterate()  # past the warm-up: the policy moves too
            self.assertNotIn("v_only", rec)
            moved = {n for n, p in ln.net.named_parameters() if not torch.equal(p, before[n])}
            self.assertIn("cstats.proj.weight", moved)
            self.assertTrue(moved - names)
            ln.save()
            ln.close()
            # A resume rebuilds the same net (the table comes back from the checkpoint).
            again = T.Learner(T.Config.from_dict(T.dataclasses.asdict(cfg)), out, resume=True)
            self.assertTrue(torch.equal(again.net.cstats.table, ln.net.cstats.table))
            again.close()


if __name__ == "__main__":
    unittest.main()
