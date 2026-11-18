"""RLN40 part B: why the net never Smiths (measurement).

    python python/rln40_smith.py probe CKPT --weights ema0.999 --n 500 --out F.json [--threads 4]

Greedy runs on DEV seeds; at every rest site records HP fraction, the policy's P(option), the greedy
choice and floor, and, by copying the env (`Sts2RunEnvs.copy`), plays each of Rest and Smith to the
next map decision (the net greedy in between: the upgrade picker) and reads V there.
Writes one record per rest visit.
"""

import argparse
import json
import pathlib
import sys

import numpy as np
import torch

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

from sts2 import E2E, HOLD, Sts2RunEnvs  # noqa: E402
from sts2.e2e import encode as E  # noqa: E402

import e2e_eval as V  # noqa: E402

REST_BASE = 236  # 140 combat + ancient 8 + event 8 + node 8 + claim 8 + pick 40 + alt 16 + skip 8
PHASE_MAP, PHASE_REST = 2, 7
REST_AT = E.REST_AT
KIND_HEAL, KIND_SMITH = 0, 1


def make_env(n, seeds, net, threads):
    v2, full, events = (bool(getattr(net, k, False)) for k in ("obs_v2", "obs_full", "obs_events"))
    return Sts2RunEnvs(n, seeds=np.asarray(seeds, dtype=np.uint64), character=V.CHARACTER,
                       ascension=V.ASCENSION, combat="agent", threads=threads, priv=True,
                       **(dict(obs_v2=True) if v2 else {}), **(dict(obs_full=True) if full else {}),
                       **(dict(obs_events=True) if events else {}), **getattr(net, "env_opts", {}),
                       **E2E), (v2, full, events)


def header(obs):
    """phase, total floor, hp, max hp of each row of `obs`."""
    o = obs.astype(np.int64)
    return o[:, 3], o[:, 7], o[:, 8] | (o[:, 9] << 8), o[:, 10] | (o[:, 11] << 8)


def rest_kinds(obs_row):
    k = obs_row[REST_AT:REST_AT + 16].reshape(8, 2)
    return [(int(a) - 1, int(b)) for a, b in k if a > 0]


@torch.no_grad()
def forward(net, env, idx, flags, device):
    v2, full, events = flags
    obs = torch.from_numpy(env.obs[idx]).to(device)
    masks = torch.from_numpy(env.masks.view(np.int64)[idx]).to(device)
    tok = E.encode(obs, masks, v2=v2, full=full, events=events,
                   discard=bool(getattr(net, "env_opts", {}).get("discard_potion")))
    ptok = E.encode_priv(torch.from_numpy(env.priv[idx].copy()).to(device))
    with torch.autocast(device.type, dtype=torch.bfloat16, enabled=device.type == "cuda"):
        logits, v = net(tok, ptok)
    return logits.float(), v.float()


def probe(args):
    device = torch.device(args.device)
    net = V.load_policy(args.ckpt, args.weights, device)
    first, count = V.PANELS["DEV-1000"]
    n = args.n
    seeds = list(range(first, first + n))
    env, flags = make_env(3 * n, seeds * 3, net, args.threads)
    A, B = n, 2 * n  # scratch: Rest at n.., Smith at 2n..
    done = np.zeros(n, dtype=bool)
    floors = np.zeros(n, dtype=np.int64)
    records = []
    steps = 0

    def greedy(idx_all):
        idx = np.asarray(idx_all)
        logits, v = forward(net, env, idx, flags, device)
        return logits.argmax(-1).cpu().numpy().astype(np.uint16), logits, v

    try:
        while not done.all():
            live = np.nonzero(~done)[0]
            act, logits, _ = greedy(live)
            ph, fl, hp, mhp = header(env.obs[live])
            rest = ph == PHASE_REST
            if rest.any():
                r = live[rest]
                lg = logits[torch.from_numpy(rest).to(device)]
                probs = torch.softmax(lg, -1).cpu().numpy()
                # options -> (kind, action index) for those with a bit
                plan = []
                for j, i in enumerate(r):
                    ks = rest_kinds(env.obs[i])
                    opts = {kind: REST_BASE + s for s, (kind, en) in enumerate(ks) if en}
                    plan.append(opts)
                both = [j for j, o in enumerate(plan) if KIND_HEAL in o and KIND_SMITH in o]
                v_after = {}
                if both:
                    sel = np.asarray([r[j] for j in both])
                    src = sel.astype(np.uint32)
                    env.copy(src, (src + A).astype(np.uint32))
                    env.copy(src, (src + B).astype(np.uint32))
                    for off, kind in ((A, KIND_HEAL), (B, KIND_SMITH)):
                        step = np.full(3 * n, HOLD, dtype=np.uint16)
                        for j in both:
                            step[r[j] + off] = plan[j][kind]
                        env.step(step)
                        pending = {int(r[j] + off) for j in both}
                        for _ in range(60):
                            sc = np.asarray(sorted(pending), dtype=np.int64)
                            if not len(sc):
                                break
                            ph2 = env.obs[sc].astype(np.int64)[:, 3]
                            fin = env.done[sc].astype(bool)
                            ok = (ph2 == PHASE_MAP) | fin
                            for e in sc[ok]:
                                pending.discard(int(e))
                            sc = np.asarray(sorted(pending), dtype=np.int64)
                            if not len(sc):
                                break
                            a, _, _ = greedy(sc)
                            step = np.full(3 * n, HOLD, dtype=np.uint16)
                            step[sc] = a
                            env.step(step)
                        for j in both:
                            e = int(r[j] + off)
                            if env.done[e]:
                                v_after[(j, kind)] = None
                                continue
                            _, v = forward(net, env, np.asarray([e]), flags, device)
                            v_after[(j, kind)] = float(v[0])
                for j, i in enumerate(r):
                    o = plan[j]
                    pl = {int(k): float(probs[j][a]) for k, a in o.items()}
                    # `r` indexes rows of `live[rest]`; `ph/hp` are over `live`
                    pos = int(np.nonzero(live == i)[0][0])
                    records.append(dict(seed=seeds[i], floor=int(fl[pos]), hp=int(hp[pos]),
                                        max_hp=int(mhp[pos]), p=pl,
                                        chose=int(act[pos]),
                                        v_rest=v_after.get((j, KIND_HEAL)),
                                        v_smith=v_after.get((j, KIND_SMITH))))
            step = np.full(3 * n, HOLD, dtype=np.uint16)
            step[live] = act
            env.step(step)
            for i in live[env.done[live].astype(bool)]:
                done[i] = True
                floors[i] = int(env.floor[i])
            steps += 1
    finally:
        env.close()
    pathlib.Path(args.out).write_text(json.dumps(dict(ckpt=str(args.ckpt), weights=args.weights,
                                                      n=n, floors=floors.tolist(),
                                                      records=records)))
    print(f"{n} runs, {len(records)} rest visits, mean floor {floors.mean():.2f}")


def band(rec):
    return min(int(10 * rec["hp"] / max(1, rec["max_hp"])), 9)


def summarise(path):
    d = json.loads(pathlib.Path(path).read_text())
    rs = d["records"]
    both = [r for r in rs if str(KIND_SMITH) in r["p"] and str(KIND_HEAL) in r["p"]]
    print(f"{path}: {len(rs)} visits, {len(both)} with Smith+Rest")
    print("| HP band | visits | P(Smith) mean | chose Smith | V(Smith)-V(Rest) mean | V pref Smith |")
    print("|---|---:|---:|---:|---:|---:|")
    for lo in (0, 3, 5, 7, 9):
        hi = {0: 3, 3: 5, 5: 7, 7: 9, 9: 10}[lo]
        g = [r for r in both if lo <= band(r) < hi]
        if not g:
            continue
        ps = [r["p"][str(KIND_SMITH)] for r in g]
        ch = [r["chose"] == REST_BASE + 1 or False for r in g]
        dv = [r["v_smith"] - r["v_rest"] for r in g if r["v_smith"] is not None and r["v_rest"] is not None]
        print(f"| {lo*10}-{hi*10 - 1 if hi<10 else 100}% | {len(g)} | {np.mean(ps):.3f} | "
              f"{np.mean([r['chose'] != REST_BASE for r in g]):.3f} | {np.mean(dv):+.4f} | "
              f"{np.mean(np.array(dv) > 0):.2f} |" if dv else f"| {lo} | {len(g)} | - |")


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("probe")
    p.add_argument("ckpt")
    p.add_argument("--weights", default="ema0.999")
    p.add_argument("--n", type=int, default=500)
    p.add_argument("--out", required=True)
    p.add_argument("--threads", type=int, default=4)
    p.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    s = sub.add_parser("summary")
    s.add_argument("files", nargs="+")
    a = ap.parse_args()
    if a.cmd == "probe":
        probe(a)
    else:
        for f in a.files:
            summarise(f)
