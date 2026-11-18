"""The obs v2 inputs (RLN16): `python -m unittest discover -s python/tests` (after
`cargo build --profile ffi -p sts2-ffi`).

On the CPU: the v1 tokens do not change with `v2`, the v2 extras survive the token cache, the
encoder's route summaries and knapsack match direct references, a v1 net's weights in a v2 net
give bit-identical logits and values (and a nonzero gate does not), and a v1 checkpoint resumed
with `obs_v2` keeps its outputs at the swap, then trains its new weights off zero; a v2 learner
killed and resumed reproduces the next iterations.
"""

import itertools
import pathlib
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import numpy as np  # noqa: E402
import torch  # noqa: E402

import train_e2e as T  # noqa: E402
from sts2 import E2E, Sts2RunEnvs  # noqa: E402
from sts2.e2e import encode as E  # noqa: E402
from sts2.e2e.net import E2ENet  # noqa: E402
from test_e2e_learner import tiny_config  # noqa: E402

NAMES = {E.PT_UNKNOWN: "Unknown", E.PT_SHOP: "Shop", E.PT_TREASURE: "Treasure",
         E.PT_REST: "RestSite", E.PT_MONSTER: "Monster", E.PT_ELITE: "Elite"}


def v2_states(n=800, envs=32, seed=0):
    env = Sts2RunEnvs(envs, seed_range=(23_000_000, 23_100_000), ascension=10, threads=2,
                      priv=True, obs_v2=True, **E2E)
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


def ref_route(o):
    """spire-mentor's `_route` and `_paths_through` on one row, with dicts."""
    nodes, boss_row = {}, 0
    for i in range(96):
        c, r, t, f, ch = (int(x) for x in o[E.MAP_AT + 5 * i:E.MAP_AT + 5 * i + 5])
        if t == E.PT_BOSS:
            boss_row = max(boss_row, r)
        if t in NAMES and r < E.GRID_ROWS:
            nodes[(c, r)] = dict(type=NAMES[t], ch=ch, nxt=(f >> 2) & 1)
    memo = {}
    for key in sorted(nodes, key=lambda k: -k[1]):
        n = nodes[key]
        typ = n["type"]
        e, r = float(typ == "Elite"), float(typ == "RestSite")
        sh, un = float(typ == "Shop"), float(typ == "Unknown")
        fight = float(typ in ("Monster", "Elite"))
        ks = [memo[(c, key[1] + 1)] for c in range(8)
              if (n["ch"] >> c) & 1 and (c, key[1] + 1) in memo]
        if ks:
            v = (e + min(k[0] for k in ks), e + max(k[1] for k in ks),
                 r + min(k[2] for k in ks), r + max(k[3] for k in ks),
                 sh + max(k[4] for k in ks), un + max(k[5] for k in ks),
                 0.0 if r else fight + min(k[6] for k in ks))
        else:
            v = (e, e, r, r, sh, un, 0.0 if r else fight)
        memo[key] = v + (float(boss_row - key[1]),)
    cnt = {}
    for key in sorted(nodes, key=lambda k: k[1]):
        cnt[key] = cnt.get(key, 0) + nodes[key]["nxt"]
        for c in range(8):
            k2 = (c, key[1] + 1)
            if (nodes[key]["ch"] >> c) & 1 and k2 in nodes:
                cnt[k2] = cnt.get(k2, 0) + cnt[key]
    reach = sorted((k for k in nodes if cnt.get(k, 0) > 0), key=lambda k: (k[1], k[0]))
    return memo, cnt, reach[:E.MAPNODES]


def i16(o, at):
    return int(np.int16(int(o[at]) | int(o[at + 1]) << 8))


class Encoder(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.obs, cls.masks, cls.priv = v2_states()

    def test_v1_tokens_do_not_change(self):
        a, b = E.encode(self.obs, self.masks), E.encode(self.obs, self.masks, v2=True)
        for k, v in a.items():
            self.assertTrue(torch.equal(v, b[k]) if torch.is_tensor(v) else v == b[k], k)
        self.assertTrue(b["xnum"].any() and b["xcat"].any() and b["link"].any())
        # Rows without the section (an ABI-6/7 library, or `obs_v2` off) are v1 rows.
        self.assertTrue(torch.equal(E.encode(self.obs[:, :E.OBS_BYTES], self.masks)["num"],
                                    a["num"]))

    def test_pack_round_trip(self):
        tok, ptok = E.encode(self.obs, self.masks, v2=True), E.encode_priv(self.priv)
        t = int(tok["width"].max())
        got, _ = E.unpack(E.pack(tok, ptok), self.masks, t)
        for k in ("link", "xcat", "xnum", "znum"):
            self.assertTrue(torch.equal(got[k], tok[k][:, :t]), k)
        for k in ("y_valid", "y_cat", "y_num", "anum"):
            self.assertTrue(torch.equal(got[k], tok[k]), k)
        self.assertFalse(tok["xnum"][~tok["valid"]].any())

    def test_side_columns_read_the_section(self):
        """RLN21: the facing and arm bytes reach the creature tokens, the shown numbers the
        power tokens (written into fight rows by hand: random runs rarely meet the crab)."""
        rows = np.nonzero((self.obs[:, 3] == E.PHASE_COMBAT).numpy())[0][:20]
        obs = self.obs[rows].clone()
        side, disp = E.V2_AT + E.V_SIDE, E.V2_AT + E.V_DISPLAY
        obs[:, side + 0] = 2  # the player faces left
        obs[:, side + 2 * 1 + 1] = 1  # creature slot 1 is an arm behind
        # Creature 0's first power shows 30.
        obs[:, disp], obs[:, disp + 1], obs[:, disp + 2] = 1, 30, 0
        tok = E.encode(obs, self.masks[rows], v2=True)
        for i in range(len(rows)):
            ctok = (tok["kind"][i] == E.K["creature"]).nonzero().squeeze(1)
            self.assertEqual(tok["znum"][i, ctok[0]].tolist(), [0, 1, 0, 0])
            if len(ctok) > 1:
                self.assertEqual(tok["znum"][i, ctok[1]].tolist(), [0, 0, 1, 0])
            ptok = ((tok["kind"][i] == E.K["power"]) & (tok["owner"][i] == 1)).nonzero()
            if len(ptok):
                z = tok["znum"][i, ptok[0, 0]]
                self.assertEqual(z[0].item(), 1)
                self.assertAlmostEqual(z[1].item(), float(np.log1p(30)), places=5)
        self.assertFalse(tok["znum"][~tok["valid"]].any())

    def test_plan_columns_read_the_section(self):
        """RLN22: the plan bytes the env writes become `anum` on their action indices: only
        masked combat indices, at most one choice per row, and the row's best has a zero gap."""
        tok = E.encode(self.obs, self.masks, v2=True)
        a = tok["anum"]
        self.assertEqual(tuple(a.shape[1:]), (E.ACTIONS, E.ANUM))
        scored = a[:, :, 0] == 1
        self.assertTrue(scored.any())
        self.assertFalse(a[:, E.COMBAT_ACTIONS:].any())
        self.assertFalse((scored & ~tok["legal"]).any())
        self.assertTrue(((a[:, :, 1] == 1).sum(1) <= 1).all())
        rows = scored.any(1)
        self.assertTrue((a[rows, :, 4].amax(1) == 0).all())
        self.assertTrue((a[:, :, 4] <= 0).all())
        # A hand-written entry: index 5 scored 123, chosen.
        obs = self.obs[:4].clone()
        at = E.V2_AT + E.V_PLAN
        obs[:, at:at + E.COMBAT_ACTIONS * E.PLAN_BYTES] = 0
        obs[:, at + 15], obs[:, at + 16], obs[:, at + 17] = 123, 0, 3
        got = E.encode(obs, self.masks[:4], v2=True)["anum"][:, 5]
        want = torch.tensor([1, 1, 0, float(np.log1p(12.3)), 0]).expand(4, -1)
        self.assertTrue(torch.allclose(got, want))

    def test_route_summaries_match_the_reference(self):
        maps = 0
        tok = E.encode(self.obs, self.masks, v2=True)
        for i in np.nonzero((self.obs[:, 3] == E.PHASE_MAP).numpy())[0]:
            memo, cnt, reach = ref_route(self.obs[i].numpy())
            yv, yn = tok["y_valid"][i], tok["y_num"][i]
            self.assertEqual(int(yv.sum()), len(reach))
            for j, key in enumerate(reach):
                self.assertEqual(int(yn[j, 1]), key[0])
                self.assertEqual(tuple(yn[j, 4:12].tolist()), memo[key])
                self.assertAlmostEqual(yn[j, 3].item(), float(np.log1p(cnt[key])), places=5)
            maps += 1
        self.assertGreater(maps, 10)

    def test_knapsack_matches_brute_force(self):
        tok = E.encode(self.obs, self.masks, v2=True)
        cols, hot = E._hand_play("cpu")
        legal = E.mask_bits(self.masks)
        fights = 0
        for i in np.nonzero((self.obs[:, 3] == E.PHASE_COMBAT).numpy())[0][:300]:
            o = self.obs[i].numpy()
            energy = i16(o, E.COMBAT_AT + 6)
            play = ((legal[i, cols].float() @ hot) > 0).tolist()
            cards = []
            for h in range(min(int(o[E.COMBAT_AT + 16]), 10)):
                if not play[h]:
                    continue
                cid = i16(o, E.COMBAT_AT + E.C_HAND + 2 * h) & 0xFFFF
                cost = int(np.int8(o[E.COMBAT_AT + E.C_CARDS + 6 * cid + 4]))
                hits = int(o[E.V2_AT + E.V_PREVIEW + 6 * h + 4])
                aim = [i16(o, E.V2_AT + E.V_AIMED + 2 * (h * 8 + e)) * hits for e in range(8)]
                cards.append((energy if cost < 0 else cost, aim))
            cap = min(max(energy, 0), E.ENERGY_CAP)
            ctok = (tok["kind"][i] == E.K["creature"]).nonzero().squeeze(1)
            for e in range(len(ctok)):
                best = max(sum(cards[k][1][e] for k in range(len(cards)) if s[k])
                           for s in itertools.product((0, 1), repeat=len(cards))
                           if sum(cards[k][0] for k in range(len(cards))
                                  if s[k] and cards[k][0] > 0) <= cap)
                self.assertAlmostEqual(tok["xnum"][i, ctok[e], 12].item(),
                                       float(E.symlog(torch.tensor(best))), places=5)
            fights += 1
        self.assertGreater(fights, 50)


def random_v1(d=32, layers=2):
    torch.manual_seed(0)
    net = E2ENet(d=d, layers=layers)
    for p in net.parameters():  # zero-init heads would hide differences
        torch.nn.init.normal_(p, std=0.1)
    return net


class ZeroInit(unittest.TestCase):
    def test_a_v1_net_in_a_v2_net_gives_the_same_bits(self):
        obs, masks, priv = v2_states(n=400, seed=3)
        v1 = random_v1().eval()
        v2 = E2ENet(d=32, layers=2, obs_v2=True)
        missing = v2.load_v1(v1.state_dict())
        self.assertTrue(missing and all(k.startswith("x_") for k in missing))
        v2.eval()
        ptok = E.encode_priv(priv)
        with torch.no_grad():
            l1, val1 = v1(E.encode(obs, masks), ptok)
            l2, val2 = v2(E.encode(obs, masks, v2=True), ptok)
            self.assertTrue(torch.equal(l1, l2))
            self.assertTrue(torch.equal(val1, val2))
            # The new inputs do reach the policy once their weights move.
            v2.x_v2.gate.fill_(0.5)
            v2.x_v2.link_gain.fill_(0.1)
            l3, val3 = v2(E.encode(obs, masks, v2=True), ptok)
        self.assertFalse(torch.equal(l1, l3))
        names = [n for n, _ in v2.named_parameters()]
        self.assertTrue(all(n.startswith("x_") for n in names[-len(v2.v2_parameters()):]))


class Resume(unittest.TestCase):
    def setUp(self):
        self.dir = pathlib.Path(tempfile.mkdtemp(dir=pathlib.Path(__file__).resolve()
                                                 .parents[2] / "target"))

    def tearDown(self):
        shutil.rmtree(self.dir, ignore_errors=True)

    def test_a_v1_checkpoint_resumes_with_v2_and_trains_the_new_weights(self):
        a = T.Learner(tiny_config(), self.dir / "c")
        a.iterate()
        a.save()
        obs, masks, priv = v2_states(n=200, seed=4)
        ptok = E.encode_priv(priv)
        with torch.no_grad():
            before = a.net(E.encode(obs, masks), ptok)
        a.close()
        cfg = tiny_config()
        T.set_resumed(cfg, "obs_v2", True)
        b = T.Learner(cfg, self.dir / "c", resume=True)
        # policy, value, v2, side (RLN21), plan (RLN22)
        self.assertEqual(len(b.opt.param_groups), 5)
        self.assertEqual(len(b.emas[0]), len(list(b.net.parameters())))
        with torch.no_grad():
            after = b.net(E.encode(obs, masks, v2=True), ptok)
        self.assertTrue(torch.equal(before[0], after[0]) and torch.equal(before[1], after[1]))
        zero = ("x_v2.gate", "x_v2.link_gain", "x_v2.fc.weight", "x_v2.stem.b.weight")
        for _ in range(6):  # the gate moves once a batch holds a map decision
            self.assertTrue(b.iterate()["finite"])
            moved = {n: bool(p.detach().any()) for n, p in b.net.named_parameters()
                     if n in zero}
            if all(moved.values()):
                break
        self.assertTrue(all(moved.values()), moved)
        b.save()
        b.close()
        c = T.Learner(cfg, self.dir / "c", resume=True)  # a v2 checkpoint resumes as one
        self.assertTrue(c.iterate()["finite"])
        c.close()

    def _older(self, cfg, path, prefixes, width):
        """Learner checkpoint at `path` made into an older one: without the `prefixes` inputs
        (the last groups, the last EMA entries) and with probe rows `width` wide."""
        s = torch.load(path, map_location="cpu", weights_only=False)
        strip = lambda d: {n: v for n, v in d.items()  # noqa: E731
                           if not n.startswith(prefixes)}
        k = sum(1 for n, _ in self.net_names(cfg) if n.startswith(prefixes))
        s["net"] = strip(s["net"])
        if s.get("behaviour") is not None:
            s["behaviour"] = strip(s["behaviour"])
        groups = s["opt"]["param_groups"]
        gone = {i for g in groups[-len(prefixes):] for i in g["params"]}
        s["opt"]["param_groups"] = groups[:-len(prefixes)]
        s["opt"]["state"] = {i: v for i, v in s["opt"]["state"].items() if i not in gone}
        s["emas"] = [e[:-k] for e in s["emas"]]
        if s["probe"] is not None:
            s["probe"]["obs"] = s["probe"]["obs"][:, :width]
        torch.save(s, path)
        return s

    @staticmethod
    def net_names(cfg):
        return E2ENet(d=cfg.d, layers=cfg.layers, obs_v2=True).named_parameters()

    def _resumes_older(self, prefixes, width, side, new_prefix, key):
        """An older v2 checkpoint (without the `prefixes` inputs) resumes with the outputs of
        the older net, then trains the newest input (`new_prefix`, token key `key`) off zero."""
        cfg = tiny_config(obs_v2=True)
        a = T.Learner(cfg, self.dir / "d")
        a.iterate()
        a.save()
        names = [n for n, _ in a.net.named_parameters()]
        a.close()
        later = [n.startswith(E2ENet.LATER_PREFIXES) for n in names]
        k = sum(later)
        self.assertTrue(k > 0 and all(later[-k:]))  # registered last
        path = self.dir / "d" / "state.pt"
        s = self._older(cfg, path, prefixes, width)
        ref = E2ENet(d=cfg.d, layers=cfg.layers, obs_v2=True, side=side, plan=False)
        ref.load_state_dict(s["net"])
        ref.value_first = cfg.value_first
        ref.eval()
        obs, masks, priv = v2_states(n=200, seed=5)
        ptok = E.encode_priv(priv)
        tok = E.encode(obs, masks, v2=True)
        self.assertTrue(tok["anum"].any())  # the env wrote plan columns
        with torch.no_grad():
            before = ref({n: v for n, v in tok.items()
                          if n not in (("anum",) if side else ("znum", "anum"))}, ptok)
        b = T.Learner(cfg, self.dir / "d", resume=True)
        self.assertEqual(len(b.opt.param_groups), 5)
        self.assertEqual(len(b.emas[0]), len(list(b.net.parameters())))
        b.net.eval()
        with torch.no_grad():
            after = b.net(tok, ptok)
        self.assertTrue(torch.equal(before[0], after[0]) and torch.equal(before[1], after[1]))
        self.assertEqual([id(p) for p in b.opt.param_groups[3]["params"]],
                         [id(p) for p in b.net.side_parameters()])
        self.assertEqual([id(p) for p in b.opt.param_groups[4]["params"]],
                         [id(p) for p in b.net.plan_parameters()])
        # The newest input reaches the policy once its weights move.
        module = getattr(b.net, new_prefix.rstrip("."))
        moved_tok = dict(tok, **{key: torch.ones_like(tok[key])})
        with torch.no_grad():
            module.proj.weight.fill_(0.1)
            moved = b.net(moved_tok, ptok)
            module.proj.weight.zero_()
        self.assertFalse(torch.equal(before[0], moved[0]))
        b.net.train()
        self.assertTrue(b.iterate()["finite"])
        b.save()
        b.close()
        c = T.Learner(cfg, self.dir / "d", resume=True)  # now a checkpoint with every input
        self.assertTrue(c.iterate()["finite"])
        c.close()

    def test_an_abi8_v2_checkpoint_resumes_with_the_side_inputs(self):
        """RLN21: a v2 checkpoint without `x_side` and `x_plan` (ABI 8) resumes with the same
        outputs as the ABI-8 net, then trains the side map off zero."""
        self._resumes_older((E2ENet.SIDE_PREFIX, E2ENet.PLAN_PREFIX), 3392, False,
                            E2ENet.SIDE_PREFIX, "znum")

    def test_an_abi9_v2_checkpoint_resumes_with_the_plan_column(self):
        """RLN22: a v2 checkpoint without `x_plan` (ABI 9, NECRO1 from 819M) resumes with the
        same outputs as the ABI-9 net, then trains the plan map off zero."""
        self._resumes_older((E2ENet.PLAN_PREFIX,), 3696, True, E2ENet.PLAN_PREFIX, "anum")

    def test_a_v2_learner_resumes_to_the_bit(self):
        cfg = tiny_config(obs_v2=True)
        a = T.Learner(cfg, self.dir / "a")
        a.iterate()
        a.save()
        ref = a.iterate()
        params = [p.detach().clone() for p in a.net.parameters()]
        a.close()
        b = T.Learner(cfg, self.dir / "a", resume=True)
        got = b.iterate()
        timing = ("collect_dps", "update_dps", "total_dps", "gpu_mem_gb")
        for k in ref:
            if k not in timing and not (isinstance(ref[k], float) and ref[k] != ref[k]):
                self.assertEqual(got[k], ref[k], k)
        for p, q in zip(b.net.parameters(), params):
            self.assertTrue(torch.equal(p.detach(), q))
        b.close()


if __name__ == "__main__":
    unittest.main()
