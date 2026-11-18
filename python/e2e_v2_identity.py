"""RLN16/21/22: an older checkpoint resumed into this net gives bit-identical logits and values.

`python e2e_v2_identity.py STATE.pt [--states N]`, two cases by the checkpoint:

- **v1** (RLN16): loads the checkpoint's net (v1) and the same weights into an `obs_v2` net
  (`E2ENet.load_v1`, the `x_` weights at their init);
- **older v2** (RLN21: ABI 8, no `x_side` or `x_plan` weights; RLN22: ABI 9, no `x_plan`, e.g.
  NECRO1 from 819M): the reference is that net (`side=`, `plan=False`, strict load); the
  candidate is what `train_e2e.py --resume` makes of the checkpoint (`Learner.load_weights` on
  the CPU: the net, the optimiser with its new groups, the EMAs), reading the full rows. Every
  weight set is compared: raw and each EMA, on the SMOKE rows, on `side_stress` rows (their
  fight rows with random facing, arm and shown-number bytes, which SMOKE runs rarely meet) and
  on `plan_stress` rows (fight rows with random plan column bytes).

Both run in fp32 on the CPU over the honesty SMOKE states (drawn with the v2 section) and are
compared with `torch.equal`. Prints a JSON report; exit status 0 when every logit and value is
the same bits.
"""

import argparse
import copy
import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import torch  # noqa: E402

from sts2.e2e import encode as E  # noqa: E402
from sts2.e2e.honesty import smoke_states  # noqa: E402
from sts2.e2e.net import E2ENet  # noqa: E402


def pair(net_state, d, layers):
    """(v1 net, v2 net) with the same weights, CPU fp32, eval."""
    v1 = E2ENet(d=d, layers=layers)
    v1.load_state_dict(net_state)
    v2 = E2ENet(d=d, layers=layers, obs_v2=True)
    v2.load_v1(net_state)
    return v1.eval(), v2.eval()


def _without(tok, *keys):
    return {k: v for k, v in tok.items() if k not in keys}


@torch.no_grad()
def compare(ref, new, obs, masks, priv, batch=250, ref_abi=0):
    """Rows whose logits or value differ (and how many rows ran). `ref_abi`: the reference is
    an ABI-8 v2 net (it reads the v2 tokens without `znum` and `anum`) or an ABI-9 one (without
    `anum`), else (0) a v1 net."""
    diff_logits = diff_values = 0
    for i in range(0, len(obs), batch):
        o, m, p = obs[i:i + batch], masks[i:i + batch], priv[i:i + batch]
        ptok = E.encode_priv(p)
        tok = E.encode(o, m, v2=True)
        ref_tok = {0: lambda: E.encode(o, m), 8: lambda: _without(tok, "znum", "anum"),
                   9: lambda: _without(tok, "anum")}[ref_abi]()
        l1, val1 = ref(ref_tok, ptok)
        l2, val2 = new(tok, ptok)
        diff_logits += int((l1 != l2).any(1).sum()) if not torch.equal(l1, l2) else 0
        diff_values += int((val1 != val2).sum()) if not torch.equal(val1, val2) else 0
    return dict(rows=len(obs), rows_with_other_logits=diff_logits,
                rows_with_other_value=diff_values)


def resumed(s):
    """The `train_e2e.py --resume` load of checkpoint `s` on the CPU (`Learner.load_weights`,
    without the envs): (the net, its EMAs as weight lists, the optimiser)."""
    import train_e2e as T

    cfg = T.Config.from_dict(s["config"])
    cfg.device, cfg.fused_adam = "cpu", False
    learner = T.Learner.__new__(T.Learner)
    learner.cfg, learner.dev = cfg, torch.device("cpu")
    learner.net = E2ENet(d=cfg.d, layers=cfg.layers, obs_v2=cfg.obs_v2)
    learner.opt = learner._optimizer(v2_group=cfg.obs_v2)
    learner.obs_width = E.obs_width(cfg.obs_v2)
    learner.load_weights(copy.deepcopy(s))
    return learner.net, learner.emas, learner.opt


def with_weights(net, weights):
    out = copy.deepcopy(net)
    with torch.no_grad():
        for p, t in zip(out.parameters(), weights, strict=True):
            p.copy_(t)
    return out.eval()


def side_stress(obs, seed=0):
    """The fight rows of `obs` with random side bytes (facing, arms, every power slot's shown
    number), and their indices."""
    rows = (obs[:, 3] == E.PHASE_COMBAT).nonzero().squeeze(1)
    o = obs[rows].clone()
    g = torch.Generator().manual_seed(seed)
    n = len(rows)
    at = E.V2_AT + E.V_SIDE
    for c in range(8):
        o[:, at + 2 * c] = torch.randint(0, 3, (n,), generator=g, dtype=torch.uint8)
        o[:, at + 2 * c + 1] = torch.randint(0, 3, (n,), generator=g, dtype=torch.uint8)
    at = E.V2_AT + E.V_DISPLAY
    for k in range(96):
        o[:, at + 3 * k] = torch.randint(0, 2, (n,), generator=g, dtype=torch.uint8)
        o[:, at + 3 * k + 1] = torch.randint(0, 256, (n,), generator=g, dtype=torch.uint8)
    return o, rows


def plan_stress(obs, seed=1):
    """The fight rows of `obs` with random plan column bytes (any score, any flags), and their
    indices."""
    rows = (obs[:, 3] == E.PHASE_COMBAT).nonzero().squeeze(1)
    o = obs[rows].clone()
    g = torch.Generator().manual_seed(seed)
    n = len(rows)
    at = E.V2_AT + E.V_PLAN
    for k in range(E.COMBAT_ACTIONS):
        for j in range(E.PLAN_BYTES):
            o[:, at + E.PLAN_BYTES * k + j] = torch.randint(0, 256, (n,), generator=g,
                                                           dtype=torch.uint8)
    return o, rows


def check_v2(s, obs, masks, priv):
    """RLN21/22: the ABI-8 or ABI-9 v2 checkpoint `s` against its resume (module doc)."""
    cfg = s["config"]
    if any(k.startswith(E2ENet.PLAN_PREFIX) for k in s["net"]):
        raise SystemExit("already a checkpoint with the plan column")
    side = any(k.startswith(E2ENet.SIDE_PREFIX) for k in s["net"])
    ref = E2ENet(d=cfg["d"], layers=cfg["layers"], obs_v2=True, side=side, plan=False)
    ref.load_state_dict(s["net"])
    net, emas, opt = resumed(s)
    sets = [("raw", ref.eval(), net.eval())]
    for decay, ref_w, new_w in zip(cfg["ema"], s["emas"], emas, strict=True):
        sets.append((f"ema{decay}", with_weights(ref, ref_w), with_weights(net, new_w)))
    rep = {}
    so, rows = side_stress(obs)
    po, prow = plan_stress(obs)
    for name, a, b in sets:
        abi = 9 if side else 8
        rep[name] = compare(a, b, obs, masks, priv, ref_abi=abi)
        rep[name + "+side_stress"] = compare(a, b, so, masks[rows], priv[rows], ref_abi=abi)
        rep[name + "+plan_stress"] = compare(a, b, po, masks[prow], priv[prow], ref_abi=abi)
    ok = all(r["rows_with_other_logits"] == 0 and r["rows_with_other_value"] == 0
             for r in rep.values())
    return dict(weights=rep, ok=ok, optimiser_groups=[len(g["params"]) for g in opt.param_groups],
                side_params=sum(p.numel() for p in net.side_parameters()),
                plan_params=sum(p.numel() for p in net.plan_parameters()),
                side_out_zero=bool((net.x_side.proj.weight == 0).all()),
                plan_out_zero=bool((net.x_plan.proj.weight == 0).all()),
                mode="abi9-v2 -> plan" if side else "abi8-v2 -> side, plan")


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("state")
    ap.add_argument("--states", type=int, default=2000)
    args = ap.parse_args(argv)
    torch.set_num_threads(4)
    s = torch.load(args.state, map_location="cpu", weights_only=False)
    cfg = s["config"]
    obs, masks, priv, counts = smoke_states(args.states, obs_v2=True)
    side = obs[:, E.V2_AT + E.V_SIDE:E.V2_AT + E.V_PLAN]
    plan = obs[:, E.V2_AT + E.V_PLAN:]
    if cfg.get("obs_v2"):
        rep = check_v2(s, obs, masks, priv)
    else:
        v1, v2 = pair(s["net"], cfg["d"], cfg["layers"])
        rep = compare(v1, v2, obs, masks, priv)
        rep["ok"] = rep["rows_with_other_logits"] == 0 and rep["rows_with_other_value"] == 0
        rep["mode"] = "v1 -> v2"
    rep.update(checkpoint=args.state, decisions=s["decisions"], strata=counts,
               rows=len(obs), rows_with_v2_bytes=int((obs[:, E.V2_AT:] != 0).any(1).sum()),
               rows_with_side_bytes=int((side != 0).any(1).sum()),
               rows_with_plan_bytes=int((plan != 0).any(1).sum()))
    print(json.dumps(rep))
    return 0 if rep["ok"] else 1


if __name__ == "__main__":
    sys.exit(main())
