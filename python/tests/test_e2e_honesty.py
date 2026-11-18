"""The e2e honesty gate H2-H4 (`docs/plans/necro-e2e.md` §5, `sts2/e2e/honesty.py`):
`python -m unittest discover -s python/tests` (after `cargo build --profile ffi -p sts2-ffi`).

On a random-init net (CPU fp32) and, when present, on the LC checkpoint (CPU fp32 and CUDA
bf16): logits bit-identical under real, permuted, blindfolded, shuffled and NaN priv; no policy
gradient into priv; greedy play unchanged by a resampled priv. A net that does leak priv into
its logits must fail H2 and H3 (a negative control, so the gate can see a leak).
"""

import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

import torch  # noqa: E402

from sts2.e2e import honesty as H  # noqa: E402
from sts2.e2e.net import E2ENet  # noqa: E402

LC = pathlib.Path("~/spirebot/target/rl-checkpoints/RLN5/lc1/state.pt").expanduser()
_STATES = {}


def states():
    if "s" not in _STATES:
        _STATES["s"] = H.smoke_states(2000)
    return _STATES["s"]


class Leaky(E2ENet):
    """Control: the policy logits read a privileged number."""

    def forward(self, tok, ptok):
        logits, v = super().forward(tok, ptok)
        leak = torch.nan_to_num(ptok["num"].float().mean((1, 2)), nan=7.0)
        return logits + leak.unsqueeze(1), v


class Honesty(unittest.TestCase):
    def check_net(self, net, device):
        obs, masks, priv, counts = states()
        self.assertEqual(len(obs), 2000)
        self.assertGreater(min(v for k, v in counts.items() if k != "act3_boss"), 100)
        r2 = H.h2(net, obs, masks, priv, device)
        r3 = H.h3(net, obs, masks, priv, device)
        r4 = H.h4(net, device)
        self.assertTrue(r2["ok"], r2)
        self.assertTrue(r3["ok"], r3)
        self.assertTrue(r4["ok"], r4)
        self.assertEqual(r4["finished"], r4["seeds"])

    def test_random_init_cpu(self):
        self.check_net(H.load_net(None, torch.device("cpu"), random_init=True),
                       torch.device("cpu"))

    @unittest.skipUnless(LC.exists(), "no LC checkpoint")
    def test_lc_cpu(self):
        self.check_net(H.load_net(LC, torch.device("cpu")), torch.device("cpu"))

    @unittest.skipUnless(LC.exists() and torch.cuda.is_available(), "no LC checkpoint or CUDA")
    def test_lc_cuda_bf16(self):
        torch.use_deterministic_algorithms(True)
        dev = torch.device("cuda")
        self.check_net(H.load_net(LC, dev), dev)

    def test_a_leaking_net_fails(self):
        obs, masks, priv, _ = states()
        torch.manual_seed(0)
        net = Leaky(d=32, layers=1)
        for p in net.parameters():
            if p.dim() >= 1:
                torch.nn.init.normal_(p, std=0.05)
        dev = torch.device("cpu")
        self.assertFalse(H.h2(net, obs[:300], masks[:300], priv[:300], dev)["ok"])
        self.assertFalse(H.h3(net, obs, masks, priv, dev)["ok"])


if __name__ == "__main__":
    unittest.main()
