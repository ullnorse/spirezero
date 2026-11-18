"""RLN32: the discard-potion actions through the binding. Off, no discard index is ever set and
the net's logits keep their 355-wide shape; on, discards appear on a rewards screen or shop with a
full belt, the encoder points each at its belt token, and the net scores them."""

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
from sts2.env import E2E, Sts2RunEnvs  # noqa: E402

DISCARD = 355
BELT_SLOTS = 6


def play(env, steps, rng):
    seen = np.zeros(E.ACTIONS_ALL, dtype=np.int64)
    for _ in range(steps):
        legal = env.legal()
        seen += legal.sum(0)
        env.step(np.where(legal, rng.random(legal.shape), -1).argmax(1))
    return seen


class Discard(unittest.TestCase):
    def test_off_lists_none_on_lists_some(self):
        kw = dict(seed_range=(32_000_000, 32_000_100), ascension=10, combat="greedy", threads=2)
        off = Sts2RunEnvs(8, **kw, **E2E)
        on = Sts2RunEnvs(8, discard_potion=True, **kw, **E2E)
        self.assertEqual(int(off.layout.actions), 361)
        seen_off = play(off, 4000, np.random.default_rng(0))
        seen_on = play(on, 4000, np.random.default_rng(0))
        self.assertEqual(seen_off[DISCARD:].sum(), 0)
        self.assertGreater(seen_on[DISCARD:].sum(), 0)
        self.assertEqual(seen_on[DISCARD + BELT_SLOTS:].sum(), 0)

    def test_the_encoder_points_a_discard_at_the_belt_and_the_net_scores_it(self):
        env = Sts2RunEnvs(8, seed_range=(32_000_000, 32_000_100), ascension=10, combat="greedy",
                          threads=2, discard_potion=True, priv=True, obs_v2=True, **E2E)
        rng = np.random.default_rng(1)
        net = T.net_of(dict(d=32, layers=1, obs_v2=True, discard_potion=True)).eval()
        self.assertEqual(net.n_actions, 361)
        checked = 0
        for _ in range(4000):
            legal = env.legal()
            if legal[:, DISCARD:].any():
                tok = E.encode(torch.as_tensor(env.obs), torch.as_tensor(env.masks.astype(np.int64)),
                               v2=True, discard=True)
                row = int(legal[:, DISCARD:].any(1).argmax())
                for s in range(BELT_SLOTS):
                    if legal[row, DISCARD + s]:
                        self.assertGreaterEqual(int(tok["ent"][row, DISCARD + s]), 0)
                with torch.no_grad():
                    logits, _, _ = net.policy(tok)
                self.assertEqual(logits.shape, (8, 361))
                self.assertTrue(torch.isfinite(logits[row, DISCARD:][
                    torch.as_tensor(legal[row, DISCARD:])]).all())
                checked += 1
                if checked >= 3:
                    break
            env.step(np.where(legal, rng.random(legal.shape), -1).argmax(1))
        self.assertGreater(checked, 0, "no discard was offered")

    def test_a_net_without_the_option_keeps_355_logits(self):
        net = T.net_of(dict(d=32, layers=1))
        self.assertEqual(net.n_actions, 355)
        a = T.net_of(dict(d=32, layers=1, discard_potion=True))
        self.assertEqual([n for n, _ in net.named_parameters()],
                         [n for n, _ in a.named_parameters()])


if __name__ == "__main__":
    unittest.main()
