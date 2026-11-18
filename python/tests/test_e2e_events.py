"""The RLN33 event section: `python -m unittest discover -s python/tests` (after
`cargo build --profile ffi -p sts2-ffi`).

On the CPU: with `events` off the rows' event section changes nothing; the vectors reach the
ancient and event option tokens only, each its own option; pack/unpack keeps them; an
event-section net runs forward and its new weights get gradient; off, the net is the v2 net.
"""

import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

import numpy as np  # noqa: E402
import torch  # noqa: E402

from sts2 import E2E, Sts2RunEnvs  # noqa: E402
from sts2.e2e import encode as E  # noqa: E402
from sts2.e2e.net import E2ENet  # noqa: E402

_STATES = {}


def event_states(n=1500, envs=48, seed=0):
    """Random-play rows with the event section (A10), cached per process; every env's first
    row is Neow's page."""
    if (n, seed) in _STATES:
        return _STATES[(n, seed)]
    env = Sts2RunEnvs(envs, seed_range=(24_000_000, 24_100_000), ascension=10, threads=2,
                      priv=True, obs_v2=True, obs_events=True, **E2E)
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
    _STATES[(n, seed)] = cat(obs), cat(masks), cat(privs)
    return _STATES[(n, seed)]


OPTION_KINDS = (E.K["ancient_option"], E.K["event_option"])


class Encoder(unittest.TestCase):
    def test_off_ignores_the_event_section(self):
        obs, masks, _ = event_states()
        self.assertEqual(obs.shape[1], E.OBS_BYTES_EVENTS)
        self.assertTrue(obs[:, E.EVENTS_AT:].any())
        a = E.encode(obs, masks, v2=True)
        b = E.encode(obs[:, :E.OBS_BYTES_V2].contiguous(), masks, v2=True)
        self.assertEqual(a.keys(), b.keys())
        for k in a:
            if torch.is_tensor(a[k]):
                self.assertTrue(torch.equal(a[k], b[k]), k)
        self.assertNotIn("evec", a)

    def test_events_need_v2(self):
        obs, masks, _ = event_states()
        with self.assertRaises(ValueError):
            E.encode(obs, masks, events=True)

    def test_slots_point_option_tokens_at_their_option(self):
        obs, masks, _ = event_states()
        tok = E.encode(obs, masks, v2=True, events=True)
        self.assertEqual(tuple(tok["evec"].shape[1:]), (8, E.EV_COLS))
        is_opt = tok["valid"] & sum(tok["kind"] == k for k in OPTION_KINDS).bool()
        slot = tok["ev_slot"]
        self.assertTrue(torch.equal(slot > 0, is_opt), "a slot off an option token")
        # Option token k reads option k (num column 1 is the option index), whose bytes are the
        # section's k-th entry.
        self.assertTrue(torch.equal(slot[is_opt] - 1, tok["num"][..., 1][is_opt].long()))
        for i, t in zip(*is_opt.nonzero(as_tuple=True), strict=True):
            k = int(slot[i, t]) - 1
            at = E.EVENTS_AT + E.EV_OPT * k
            raw = int(obs[i, at]) | (int(obs[i, at + 1]) << 8)
            self.assertEqual(float(tok["evec"][i, k, 0]), float(E.symlog(torch.tensor(raw))))
        # Most options met are fully described, events' as well as Neow's.
        known = torch.gather(tok["evec"][..., 0], 1, (slot - 1).clamp(min=0)) > 0
        self.assertGreater(int((known & is_opt).sum()), 0.8 * int(is_opt.sum()))
        self.assertGreater(int((known & (tok["kind"] == E.K["event_option"])).sum()), 5)
        self.assertLessEqual(int(tok["eids"].max()), E.ID_ROWS - 1)

    def test_pack_round_trip(self):
        obs, masks, priv = event_states()
        tok = E.encode(obs, masks, v2=True, events=True)
        packed = E.pack(tok, E.encode_priv(priv))
        shapes = E.packed_shapes(torch.float32, v2=True, events=True)
        self.assertEqual(packed.keys(), shapes.keys())
        for k, (shape, dtype) in shapes.items():
            self.assertEqual(tuple(packed[k].shape[1:]), shape, k)
            self.assertEqual(packed[k].dtype, dtype, k)
        width = int(tok["width"].max())
        back, _ = E.unpack(packed, masks, width)
        self.assertTrue(torch.equal(back["ev_slot"], tok["ev_slot"][:, :width]))
        for k in ("evec", "eids", "ench"):
            self.assertTrue(torch.equal(back[k], tok[k].to(back[k].dtype)), k)


class Net(unittest.TestCase):
    def test_forward_and_gradient(self):
        obs, masks, priv = event_states()
        torch.manual_seed(0)
        net = E2ENet(d=32, layers=1, obs_v2=True, obs_events=True, card_stats=True)
        for p in net.parameters():  # the zero-init heads would stop the gradient
            torch.nn.init.normal_(p, std=0.1)
        logits, value = net(E.encode(obs, masks, v2=True, events=True), E.encode_priv(priv))
        self.assertTrue(torch.isfinite(logits[torch.isfinite(logits)]).all())
        self.assertTrue(torch.isfinite(value).all())
        logp = torch.log_softmax(logits.float(), -1)
        loss = -logp.masked_fill(~torch.isfinite(logp), 0).sum() + value.sum()
        loss.backward()
        self.assertGreater(float(net.events.proj.weight.grad.abs().sum()), 0)

    def test_off_keeps_the_v2_net(self):
        torch.manual_seed(0)
        a = E2ENet(d=32, layers=1, obs_v2=True)
        torch.manual_seed(0)
        b = E2ENet(d=32, layers=1, obs_v2=True, obs_events=False)
        self.assertEqual([n for n, _ in a.named_parameters()],
                         [n for n, _ in b.named_parameters()])
        for (_, p), (_, q) in zip(a.named_parameters(), b.named_parameters(), strict=True):
            self.assertTrue(torch.equal(p, q))
        torch.manual_seed(0)
        c = E2ENet(d=32, layers=1, obs_v2=True, obs_events=True)
        names = [n for n, _ in c.named_parameters()]
        self.assertEqual(names[:len(list(a.parameters()))], [n for n, _ in a.named_parameters()])
        for (_, p), (_, q) in zip(a.named_parameters(), c.named_parameters(), strict=False):
            self.assertTrue(torch.equal(p, q))

    def test_needs_v2(self):
        with self.assertRaises(ValueError):
            E2ENet(d=32, layers=1, obs_events=True)


if __name__ == "__main__":
    unittest.main()
