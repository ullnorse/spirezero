"""The e2e honesty gate, H2-H4 (`docs/plans/necro-e2e.md` §5); H1 is the Rust positive control.

    ~/spirebot/.venv/bin/python -m sts2.e2e.honesty <state.pt | dir> [--device cpu|cuda|both]
    (run from `python/`, or with `python/` on PYTHONPATH; `--random-init` checks a fresh net)

Exit status 0 only if every check passes; the report is one JSON object on stdout.

- **H2** 2,000 NB-SMOKE states, stratified by decision kind (`STRATA`; rare strata, such as an
  act-3 boss under a random policy, may fall short and are reported with their counts; the
  shortfall is filled from the other strata). The logits of the full forward `net(tok, ptok)`
  must be bit-identical (`torch.equal`) with `priv` real, permuted across rows, blindfolded
  and NaN-filled, on CPU fp32 and on CUDA bf16 with deterministic algorithms. The blindfolded
  world: `RunEnvs` has no blindfold hook, so the stand-ins are an all-zero `priv` (what a
  world with the hidden draw, moves and rooms removed shows) and a `priv` with every byte column shuffled across rows.
- **H3** the gradient of the sum of the policy logits with respect to the privileged tokens
  is exactly zero, no value-module parameter gets a gradient from it, and the policy
  parameter set shares nothing with the value module's.
- **H4** greedy play on 50 SMOKE seeds with `priv` resampled at every step (rolled across
  envs or column-shuffled) takes the same actions and ends the same way as with the real `priv`.
"""

import argparse
import json
import os
import pathlib
import sys

os.environ.setdefault("CUBLAS_WORKSPACE_CONFIG", ":4096:8")

import numpy as np  # noqa: E402
import torch  # noqa: E402

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2]))

from sts2 import E2E, HOLD, Sts2RunEnvs  # noqa: E402
from sts2.e2e import encode as E  # noqa: E402
from sts2.e2e.net import E2ENet  # noqa: E402

SMOKE_SEEDS = (23_200_000, 23_300_000)
H4_SEEDS = (23_300_000, 23_300_050)
STRATA = ("neow", "event", "map", "shop", "rest", "rewards", "combat_osty_alive",
          "combat_osty_dead", "act3_boss")


def strata_of(obs):
    """Decision kind per row (index into `STRATA`) from the observation bytes (numpy)."""
    o = obs
    phase, act, room = o[:, 3], o[:, 2], o[:, 26]
    cb = E.COMBAT_AT + E.C_CREATURES
    cr = o[:, cb:cb + 8 * E.CREATURE_BYTES].reshape(len(o), 8, E.CREATURE_BYTES)
    max_hp = cr[:, :, 8].astype(np.int16) | (cr[:, :, 9].astype(np.int16) << 8)
    ally = (cr[:, :, 1] == 0) & (cr[:, :, 0] != 0) & (max_hp > 0)
    osty = ally[:, 1:].any(1)
    out = np.full(len(o), -1, dtype=np.int64)
    ix = {k: i for i, k in enumerate(STRATA)}
    out[phase == 0] = ix["neow"]
    out[phase == 1] = ix["event"]
    out[phase == 2] = ix["map"]
    out[phase == 9] = ix["shop"]
    out[phase == 7] = ix["rest"]
    out[np.isin(phase, (4, 5, 6, 8))] = ix["rewards"]
    combat = phase == 3
    out[combat & osty] = ix["combat_osty_alive"]
    out[combat & ~osty] = ix["combat_osty_dead"]
    out[combat & (room == 3) & (act == 2)] = ix["act3_boss"]
    return out


def _v2_kw(obs_v2, obs_full=False, obs_events=False):
    """The env flags of the obs v2, full-view and event sections (left out when off: an older
    library runs)."""
    return ((dict(obs_v2=True) if obs_v2 else {}) | (dict(obs_full=True) if obs_full else {})
            | (dict(obs_events=True) if obs_events else {}))


def _full(net):
    """Whether `net` reads the full view (RLN30)."""
    return bool(getattr(net, "obs_full", False))


def _events(net):
    """Whether `net` reads the event section (RLN33)."""
    return bool(getattr(net, "obs_events", False))


def smoke_states(n=2000, envs=256, seed=0, max_steps=4000, obs_v2=False, obs_full=False,
                 obs_events=False):
    """`n` states by decision kind under a uniformly random legal policy: obs, masks, priv
    (torch CPU tensors) and the per-stratum counts. `obs_v2` adds the v2 section and `obs_full`
    the full-view one, `obs_events` the event one (the same states: they change no other
    byte)."""
    quota = -(-n // len(STRATA))
    env = Sts2RunEnvs(envs, seed_range=SMOKE_SEEDS, ascension=10, threads=4, priv=True,
                      **_v2_kw(obs_v2, obs_full, obs_events), **E2E)
    rng = np.random.default_rng(seed)
    keep = [[] for _ in STRATA]  # per stratum: list of (obs, masks, priv) row batches
    have = np.zeros(len(STRATA), dtype=np.int64)
    spare = []  # every sampled row beyond quota, for filling shortfalls
    for _ in range(max_steps):
        kind = strata_of(env.obs)
        for k in range(len(STRATA)):
            rows = np.nonzero(kind == k)[0]
            take = rows[: max(0, quota - have[k])]
            if len(take):
                keep[k].append((env.obs[take].copy(), env.masks[take].copy(),
                                env.priv[take].copy()))
                have[k] += len(take)
            rest = rows[len(take):]
            if len(rest) and len(spare) < 400:
                spare.append((env.obs[rest].copy(), env.masks[rest].copy(),
                              env.priv[rest].copy()))
        if all(have[k] >= quota for k in range(len(STRATA) - 1)):
            break
        legal = env.legal()[:, :E.ACTIONS]  # the pre-RLN32 draw shape: the same states
        env.step(np.where(legal, rng.random(legal.shape), -1).argmax(1))
    env.close()
    cat = lambda xs, j: np.concatenate([x[j] for x in xs])  # noqa: E731
    parts = [p for k in keep for p in k]
    short = n - int(have.sum())
    if short > 0 and spare:
        extra = [(cat(spare, 0)[:short], cat(spare, 1)[:short], cat(spare, 2)[:short])]
        parts += extra
    obs, masks, priv = (np.concatenate([p[j] for p in parts])[:n] for j in range(3))
    counts = dict(zip(STRATA, have.tolist()))
    return (torch.from_numpy(obs), torch.from_numpy(masks.view(np.int64)),
            torch.from_numpy(priv), counts)


def load_net(path, device, random_init=False, layers=None, d=None, obs_v2=False, obs_full=False,
             obs_events=False):
    """The net of a checkpoint (`state.pt`, or a directory holding it), or a random one."""
    if random_init:
        torch.manual_seed(0)
        net = E2ENet(d=d or 256, layers=layers or 4, obs_v2=obs_v2 or obs_full or obs_events,
                     obs_full=obs_full, obs_events=obs_events)
        for p in net.parameters():  # the zero-init heads would make every logit equal
            if p.dim() >= 1:
                torch.nn.init.normal_(p, std=0.05)
    else:
        path = pathlib.Path(path).expanduser()
        path = path / "state.pt" if path.is_dir() else path
        s = torch.load(path, map_location="cpu", weights_only=False)
        cfg = s["config"]
        net = E2ENet(d=cfg["d"], layers=cfg["layers"], obs_v2=cfg.get("obs_v2", False),
                     card_stats=cfg.get("card_stats", False),
                     obs_full=cfg.get("obs_full", False),
                     obs_events=cfg.get("obs_events", False),
                     discard_potion=cfg.get("discard_potion", False),
                     run_head=cfg.get("run_head", False))
        net.load_net(s["net"])
    return net.to(device).eval()


def _run(net, tok, ptok, device):
    with torch.no_grad(), torch.autocast(device.type, dtype=torch.bfloat16,
                                         enabled=device.type == "cuda"):
        logits, _ = net(tok, ptok)
    return logits


def _tokens(obs, masks, priv, device, v2=False, full=False, events=False):
    obs, masks, priv = obs.to(device), masks.to(device), priv.to(device)
    return E.encode(obs, masks, v2=v2, full=full, events=events), priv


def _variants(priv, gen):
    """The privileged inputs H2 compares: name -> (B, PRIV_BYTES) uint8 plus an optional
    NaN flag (uint8 bytes cannot hold NaN, so the float columns are poisoned after encoding)."""
    b = priv.shape[0]
    perm = torch.randperm(b, generator=gen).to(priv.device)
    # Every byte column shuffled on its own across rows: in-range values, broken structure.
    cols = torch.stack([torch.randperm(b, generator=gen) for _ in range(priv.shape[1])], 1)
    shuf = torch.gather(priv.cpu(), 0, cols).to(priv.device)
    return dict(real=(priv, False), permuted=(priv[perm], False),
                blindfold_zero=(torch.zeros_like(priv), False), shuffled_bytes=(shuf, False),
                nan=(priv, True))


def h2(net, obs, masks, priv, device, batch=500):
    """Bit-identity of the logits over the five privileged inputs; returns the report."""
    gen = torch.Generator().manual_seed(7)
    bad, rows = {}, 0
    for i in range(0, len(obs), batch):
        tok, p = _tokens(obs[i:i + batch], masks[i:i + batch], priv[i:i + batch], device,
                         net.obs_v2, _full(net), _events(net))
        variants = _variants(p, gen)
        out = {}
        for name, (pv, poison) in variants.items():
            ptok = E.encode_priv(pv)
            if poison:
                ptok = dict(ptok, num=torch.full_like(ptok["num"], float("nan")))
            out[name] = _run(net, tok, ptok, device)
        base = out["real"]
        for name, lg in out.items():
            if not torch.equal(lg, base):
                bad[name] = bad.get(name, 0) + int((lg != base).any(1).sum())
        rows += len(tok["legal"])
    return dict(ok=not bad, states=rows, differing_rows=bad)


def h3(net, obs, masks, priv, device, rows=300):
    tok, p = _tokens(obs[:rows], masks[:rows], priv[:rows], device, net.obs_v2, _full(net),
                     _events(net))
    ptok = E.encode_priv(p)
    ptok["num"] = ptok["num"].detach().clone().requires_grad_(True)
    net.zero_grad(set_to_none=True)
    logits, _ = net(tok, ptok)
    logits[torch.isfinite(logits)].float().sum().backward()
    g = ptok["num"].grad
    value_ids = {id(q) for q in net.value_parameters()}
    leaked = [n for n, q in net.named_parameters()
              if id(q) in value_ids and q.grad is not None and bool(q.grad.any())]
    shared = value_ids & {id(q) for q in net.policy_parameters()}
    named = [n for n, _ in net.named_parameters() if n.startswith(net.VALUE_PREFIX)]
    ok = (g is None or not bool(g.any())) and not leaked and not shared and bool(named)
    net.zero_grad(set_to_none=True)
    return dict(ok=bool(ok), priv_grad_nonzero=g is not None and bool(g.any()),
                value_params_with_policy_grad=leaked, shared_params=len(shared))


@torch.no_grad()
def _greedy(net, device, resample, steps=3000, seed=11):
    """Greedy play on the H4 seeds: per env the actions up to its first run end and the
    outcome (won, floor, end). `resample` swaps the privileged input at every step."""
    n = H4_SEEDS[1] - H4_SEEDS[0]
    env = Sts2RunEnvs(n, seed_range=H4_SEEDS, ascension=10, threads=4, priv=True,
                      **_v2_kw(net.obs_v2, _full(net), _events(net)), **E2E)
    gen = torch.Generator().manual_seed(seed)
    acts = [[] for _ in range(n)]
    result = [None] * n
    for _ in range(steps):
        obs = torch.from_numpy(env.obs.copy()).to(device)
        masks = torch.from_numpy(env.masks.view(np.int64).copy()).to(device)
        priv = torch.from_numpy(env.priv.copy()).to(device)
        if resample:
            roll = int(torch.randint(1, n, (1,), generator=gen))
            cols = torch.stack([torch.randperm(n, generator=gen) for _ in range(priv.shape[1])], 1)
            shuf = torch.gather(priv.cpu(), 0, cols).to(device)
            priv = torch.where(torch.rand(n, 1, generator=gen).to(device) < 0.5,
                               priv.roll(roll, 0), shuf)
        tok = E.encode(obs, masks, v2=net.obs_v2, full=_full(net), events=_events(net),
                       discard=net.n_actions == E.ACTIONS_ALL)
        logits = _run(net, tok, E.encode_priv(priv), device)
        a = logits.argmax(-1).cpu().numpy()
        for i in range(n):
            if result[i] is not None:
                a[i] = HOLD
            else:
                acts[i].append(int(a[i]))
        env.step(a)
        for i in np.nonzero(env.done)[0]:
            if result[i] is None:
                result[i] = (int(env.won[i]), int(env.floor[i]), int(env.end[i]))
        if all(r is not None for r in result):
            break
    env.close()
    return acts, result


def h4(net, device):
    ref = _greedy(net, device, resample=False)
    alt = _greedy(net, device, resample=True)
    same_actions = sum(a == b for a, b in zip(ref[0], alt[0]))
    same_result = sum(a == b for a, b in zip(ref[1], alt[1]))
    n = len(ref[0])
    finished = sum(r is not None for r in ref[1])
    return dict(ok=same_actions == n and same_result == n, seeds=n, finished=finished,
                same_actions=same_actions, same_outcome=same_result,
                actions=int(sum(len(a) for a in ref[0])))


def check(net, device, states=2000):
    """H2-H4 on one device; the report dict (`ok` is the conjunction)."""
    if device.type == "cuda":
        torch.use_deterministic_algorithms(True)
    obs, masks, priv, counts = smoke_states(states, obs_v2=net.obs_v2, obs_full=_full(net),
                                            obs_events=_events(net))
    rep = dict(device=str(device), precision="bf16" if device.type == "cuda" else "fp32",
               strata=counts, h2=h2(net, obs, masks, priv, device),
               h3=h3(net, obs, masks, priv, device), h4=h4(net, device))
    rep["ok"] = all(rep[k]["ok"] for k in ("h2", "h3", "h4"))
    return rep


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("checkpoint", nargs="?")
    ap.add_argument("--random-init", action="store_true")
    ap.add_argument("--device", default="both", choices=("cpu", "cuda", "both"))
    ap.add_argument("--states", type=int, default=2000)
    ap.add_argument("--obs-v2", action="store_true", help="with --random-init: a v2 net")
    ap.add_argument("--obs-full", action="store_true",
                    help="with --random-init: a full-view net (RLN30)")
    ap.add_argument("--obs-events", action="store_true",
                    help="with --random-init: an event-section net (RLN33)")
    args = ap.parse_args(argv)
    if not args.checkpoint and not args.random_init:
        ap.error("a checkpoint or --random-init")
    devices = ["cpu"] if args.device == "cpu" else ["cuda"] if args.device == "cuda" else (
        ["cpu"] + (["cuda"] if torch.cuda.is_available() else []))
    reports = []
    for dev in devices:
        d = torch.device(dev)
        net = load_net(args.checkpoint, d, random_init=args.random_init, obs_v2=args.obs_v2,
                       obs_full=args.obs_full, obs_events=args.obs_events)
        reports.append(check(net, d, args.states))
    ok = all(r["ok"] for r in reports)
    print(json.dumps(dict(ok=ok, checkpoint=args.checkpoint, reports=reports)))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
