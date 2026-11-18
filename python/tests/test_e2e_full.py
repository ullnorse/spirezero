"""The RLN30 full view: `python -m unittest discover -s python/tests` (after
`cargo build --profile ffi -p sts2-ffi`).

On the CPU: with `full` off the rows' full section changes nothing; the pile tokens are the
pile cards in a canonical order, so reordering the card table's pile slots (the draw order
among them) leaves them the same; the past moves reach the creature tokens; pack/unpack keeps
every full-view tensor; a full-view net runs forward and its new weights get gradient, and its
learner killed and resumed reproduces the next iterations.
"""

import pathlib
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import numpy as np  # noqa: E402
import torch  # noqa: E402

from sts2 import E2E, Sts2RunEnvs  # noqa: E402
from sts2.e2e import encode as E  # noqa: E402
from sts2.e2e.net import E2ENet  # noqa: E402
import test_e2e_learner  # noqa: E402

_STATES = {}


def full_states(n=1200, envs=32, seed=0):
    """Random-play rows with the full section (A10), cached per process."""
    if (n, seed) in _STATES:
        return _STATES[(n, seed)]
    env = Sts2RunEnvs(envs, seed_range=(23_000_000, 23_100_000), ascension=10, threads=2,
                      priv=True, obs_v2=True, obs_full=True, **E2E)
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


def pile_tokens(tok, i):
    """Row `i`'s pile tokens as (ident, cat, num, counts) tuples, in token order."""
    sel = tok["valid"][i] & (tok["kind"][i] == E.K["pile"])
    return list(zip(tok["ident"][i][sel].tolist(), tok["cat"][i][sel].tolist(),
                    [tuple(r) for r in tok["num"][i][sel].tolist()],
                    [tuple(r) for r in tok["num"][i][sel][:, 9:12].tolist()], strict=True))


def far_points(obs, i):
    """Row `i`'s map points that `map_row` tokens show, as {(ahead, col): (type, children)}."""
    m = obs[i, E.MAP_AT:E.MAP_AT + 96 * 5].view(96, 5).long()
    cur = int(obs[i, 25])
    cur = -1 if cur == 255 else cur
    phase = int(obs[i, 3])
    out = {}
    for col, row, ptype, flags, child in m.tolist():
        ahead = row - cur
        if ptype == 0 or ahead <= 0 or phase == E.PHASE_COMBAT:
            continue
        if phase == E.PHASE_MAP and ahead <= E.MAP_ROWS:
            continue
        out[(ahead, col)] = (ptype, child)
    return out


class Encoder(unittest.TestCase):
    def test_full_off_ignores_the_full_section(self):
        obs, masks, _ = full_states()
        self.assertGreaterEqual(obs.shape[1], E.OBS_BYTES_FULL)  # ABI 14 appends events
        self.assertTrue(obs[:, E.FULL_AT:].any())
        a = E.encode(obs, masks, v2=True)
        b = E.encode(obs[:, :E.OBS_BYTES_V2].contiguous(), masks, v2=True)
        self.assertEqual(a.keys(), b.keys())
        for k in a:
            if torch.is_tensor(a[k]):
                self.assertTrue(torch.equal(a[k], b[k]), k)
        self.assertNotIn("hcat", a)

    def test_full_needs_v2(self):
        obs, masks, _ = full_states()
        with self.assertRaises(ValueError):
            E.encode(obs, masks, full=True)

    def test_pile_tokens_hide_the_table_order(self):
        obs, masks, _ = full_states()
        tok = E.encode(obs, masks, v2=True, full=True)
        c = E.COMBAT_AT + E.C_CARDS
        table = obs[:, c:c + 64 * 6].view(-1, 64, 6)
        kind = table[:, :, 0].long() | (table[:, :, 1].long() << 8)
        in_pile = (kind > 0) & (table[:, :, 3] >= 2) & (table[:, :, 3] <= 4)
        gen = torch.Generator().manual_seed(1)
        shuffled = obs.clone()
        rows = 0
        for i in range(len(obs)):
            slots = in_pile[i].nonzero().flatten()
            if len(slots) < 2:
                continue
            perm = slots[torch.randperm(len(slots), generator=gen)]
            t = shuffled[i, c:c + 64 * 6].view(64, 6)
            t[slots] = table[i, perm]
            rows += 1
        self.assertGreater(rows, 100)
        other = E.encode(shuffled, masks, v2=True, full=True)
        merged = 0
        for i in range(len(obs)):
            mine = pile_tokens(tok, i)
            self.assertEqual(mine, pile_tokens(other, i), i)
            # One token per distinct card (its card bytes, any pile), counted per pile.
            want = {}
            if tok["combat"][i]:
                for s_ in in_pile[i].nonzero().flatten().tolist():
                    k, u, pile, cost, fl = (int(kind[i, s_]), int(table[i, s_, 2]),
                                            int(table[i, s_, 3]), int(table[i, s_, 4]),
                                            int(table[i, s_, 5]))
                    want.setdefault((k, u, cost, fl), [0, 0, 0])[pile - 2] += 1
            self.assertEqual(len(mine), len(want), i)
            merged += int(in_pile[i].sum()) - len(want) if tok["combat"][i] else 0
            got = {}
            for ident, cat, num, counts in mine:
                n = [round(float(torch.expm1(torch.tensor(x)))) for x in counts]
                self.assertEqual(cat, sum((x > 0) << k for k, x in enumerate(n)), i)
                got[(ident - E._OFF["cards"], int(num[0]), int(num[1]) & 0xFF,
                     sum(int(num[3 + k]) << k for k in range(6)))] = n
            self.assertEqual(got, {(k, u, cost, fl): n for (k, u, cost, fl), n in want.items()},
                             i)
            # Canonical: sorted by the card's bytes, never by the table's or a pile's order.
            keys = [(m[0], m[2]) for m in mine]
            self.assertEqual(keys, sorted(keys), i)
        self.assertGreater(merged, 100)

    def test_map_rows_show_every_far_point(self):
        obs, masks, _ = full_states()
        tok = E.encode(obs, masks, v2=True, full=True)
        rows = 0
        for i in range(len(obs)):
            sel = tok["valid"][i] & (tok["kind"][i] == E.K["map_row"])
            got = {}
            for num, cells in zip(tok["num"][i][sel].tolist(), tok["hcat"][i][sel].tolist(),
                                  strict=True):
                ahead = int(num[0])
                for col in range(E.MAP_COLS):
                    t, e = cells[2 * col], cells[2 * col + 1]
                    if t == 0:
                        self.assertEqual(e, 0)
                        continue
                    got[(ahead, col)] = (t - E.MAP_TYPE_AT - col * E.MAP_TYPES,
                                         e - E.MAP_EDGE_AT - col * 16)
            want = far_points(obs, i)
            self.assertEqual(got.keys(), want.keys(), i)
            for (ahead, col), (ptype, child) in want.items():
                near = sum(((child >> (col + d)) & 1) << k for k, d in enumerate((-1, 0, 1))
                           if 0 <= col + d < 8)
                other = int(any((child >> c) & 1 for c in range(8) if abs(c - col) > 1))
                self.assertEqual(got[(ahead, col)], (ptype, near | other << 3), i)
            rows += bool(want)
            self.assertEqual(int(sel.sum()), len({a for a, _ in want}), i)
        self.assertGreater(rows, 50)

    def test_past_moves_reach_the_creature_tokens(self):
        obs, masks, _ = full_states()
        tok = E.encode(obs, masks, v2=True, full=True)
        has = tok["hmove"] > 0
        self.assertGreater(int(has.any(-1).any(-1).sum()), 50)
        self.assertTrue(bool((tok["kind"][has.any(-1)] == E.K["creature"]).all()))
        self.assertEqual(tok["hnum"].shape[-1], E.HNUM)
        self.assertEqual(tok["hcat"].shape[-1], E.HCAT)
        self.assertLessEqual(int(tok["width"].max()), E.T_MAX_FULL)

    def test_pack_round_trip(self):
        obs, masks, priv = full_states()
        tok = E.encode(obs, masks, v2=True, full=True)
        ptok = E.encode_priv(priv)
        packed = E.pack(tok, ptok)
        shapes = E.packed_shapes(torch.float32, v2=True, full=True)
        self.assertEqual(packed.keys(), shapes.keys())
        for k, (shape, dtype) in shapes.items():
            self.assertEqual(tuple(packed[k].shape[1:]), shape, k)
            self.assertEqual(packed[k].dtype, dtype, k)
        width = int(tok["width"].max())
        back, _ = E.unpack(packed, masks, width)
        for k in ("hcat", "hmove", "hnum", "num", "valid", "kind", "ident", "cat"):
            self.assertTrue(torch.equal(back[k], tok[k][:, :width].to(back[k].dtype)), k)
        self.assertTrue(torch.equal(back["ent"], tok["ent"].long()))
        self.assertTrue(torch.equal(back["tgt"], tok["tgt"].long()))


class Net(unittest.TestCase):
    def test_forward_and_gradient(self):
        obs, masks, priv = full_states(n=256)
        torch.manual_seed(0)
        net = E2ENet(d=32, layers=1, obs_v2=True, obs_full=True)
        for p in net.parameters():  # the zero-init heads would stop the gradient
            torch.nn.init.normal_(p, std=0.1)
        logits, value = net(E.encode(obs, masks, v2=True, full=True), E.encode_priv(priv))
        self.assertTrue(torch.isfinite(logits[torch.isfinite(logits)]).all())
        self.assertTrue(torch.isfinite(value).all())
        logp = torch.log_softmax(logits.float(), -1)
        loss = -logp.masked_fill(~torch.isfinite(logp), 0).sum() + value.sum()
        loss.backward()
        for name in ("kind", "hcat", "hmove"):
            self.assertGreater(float(getattr(net.full, name).weight.grad.abs().sum()), 0, name)

    def test_off_keeps_the_v2_net(self):
        torch.manual_seed(0)
        a = E2ENet(d=32, layers=1, obs_v2=True)
        torch.manual_seed(0)
        b = E2ENet(d=32, layers=1, obs_v2=True, obs_full=True)
        sa, sb = a.state_dict(), b.state_dict()
        self.assertTrue(all(k.startswith("full.") for k in sb.keys() - sa.keys()))
        for k in sa:
            self.assertTrue(torch.equal(sa[k], sb[k]), k)


class Learner(unittest.TestCase):
    def setUp(self):
        self.dir = pathlib.Path(tempfile.mkdtemp(dir=pathlib.Path(__file__).resolve()
                                                 .parents[2] / "target"))

    def tearDown(self):
        shutil.rmtree(self.dir, ignore_errors=True)

    def test_kill_and_resume_reproduce_the_next_iterations(self):
        cfg = test_e2e_learner.tiny_config(obs_v2=True, obs_full=True)
        test_e2e_learner.Learner.kill_and_resume(self, cfg)


if __name__ == "__main__":
    unittest.main()
