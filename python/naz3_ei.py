"""NAZ3 expert iteration: distil the search player into the combat net, then search again.

    python python/naz3_ei.py gen CKPT --count N --seed S --out DIR [search flags]
    python python/naz3_ei.py train CKPT --data DIR [DIR ...] --out CKPT2 [--epochs E --lr LR]

**gen**: training starts (NAZ1's own-run harvest `own_train` + `own_train2`, seeds 300M-300M+6k
and 320M-320M+30k; the benchmark comes from `own_test`, seeds 310M+, so no start is shared)
drawn by room with `--mix` (boss, elite, hallway shares), each played once by the search player
(`naz3_combat.SearchPlayer`). Every searched decision is stored, and `--keep` of the others:
obs and mask, the net's top-8 actions and probabilities, the search's candidates and their mean
utilities (NaN when not searched), the move played, the rollout's id; and per rollout the end
(won, HP left, max HP). Chunks of `--chunk` rows go to `DIR/part*.npz`.

**train**: from CKPT's weights (`--weights`), the whole net learns, on the stored combat rows,
a cross-entropy to a target distribution: the stored top-8 distribution of the net that played
(an anchor), where a searched row moves its candidates' mass to `softmax(log p + Q / tau)` over
the candidates (a regularised policy improvement; `tau` in utility units). A fight value head
(`FightValue`, on the trunk's first token) learns the rollout's end: P(win) by BCE and HP left /
max HP of a won fight by MSE, weight `--vf`. The result is a checkpoint `naz3_combat.py run
CKPT2 --weights raw` plays (it keeps the source config; `naz3_value` holds the value head).
"""

import argparse
import json
import os
import pathlib
import sys
import time

import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import naz3_combat as C  # noqa: E402

TOPN = 8


def train_starts(mix, count, rng):
    pool = C.read_jsonl(C.NAZ1 / "own_train.jsonl") + C.read_jsonl(C.NAZ1 / "own_train2.jsonl")
    by = {r: [x for x in pool if C.room_of(x) == r] for r in C.ROOMS}
    shares = dict(zip(("boss", "elite", "hallway"), mix))
    out = []
    for room, share in shares.items():
        n = int(round(count * share))
        out += [by[room][i] for i in rng.choice(len(by[room]), n, replace=len(by[room]) < n)]
    return [out[i] for i in rng.permutation(len(out))]


class Recorder:
    def __init__(self, out, keep, chunk, seed):
        self.out = pathlib.Path(out)
        self.out.mkdir(parents=True, exist_ok=True)
        self.keep, self.chunk = keep, chunk
        self.rng = np.random.default_rng(seed)
        self.buf = {k: [] for k in ("obs", "masks", "top_a", "top_p", "cand", "q", "act", "fight",
                                    "searched", "u")}
        self.rows = 0
        self.part = len(list(self.out.glob("part*.npz")))
        self.ends = {}

    def step(self, player, env, idx, k_of, lg, a):
        import torch

        s = player.last_searched.astype(bool)
        sel = s | (self.rng.random(len(idx)) < self.keep)
        if not sel.any():
            return
        p = torch.softmax(lg[torch.from_numpy(np.nonzero(sel)[0]).to(lg.device)], -1)
        tp = torch.topk(p, TOPN, dim=1)
        rows = idx[sel]
        b = self.buf
        b["obs"].append(env.obs[rows].copy())
        b["masks"].append(env.masks[rows].copy())
        b["top_a"].append(tp.indices.cpu().numpy().astype(np.int16))
        b["top_p"].append(tp.values.float().cpu().numpy().astype(np.float16))
        b["cand"].append(player.last_cand[sel].astype(np.int16))
        b["q"].append(player.last_q[sel])
        b["u"].append(player.last_u[sel])
        b["act"].append(a[sel].astype(np.int16))
        b["fight"].append(k_of[sel].astype(np.int64))
        b["searched"].append(s[sel])
        self.rows += int(sel.sum())
        if self.rows >= self.chunk:
            self.flush()

    def end(self, k, won, hp, max_hp):
        self.ends[k] = (won, hp, max_hp)

    def flush(self):
        if not self.rows:
            return
        z = {k: np.concatenate(v) for k, v in self.buf.items()}
        np.savez(self.out / f"part{self.part:03d}.npz", **z)
        self.part += 1
        self.buf = {k: [] for k in self.buf}
        self.rows = 0

    def close(self, offset):
        self.flush()
        ks = np.array(sorted(self.ends), dtype=np.int64)
        e = np.array([self.ends[k] for k in ks], dtype=np.int32).reshape(-1, 3)
        np.savez(self.out / f"ends{offset:09d}.npz", fight=ks, won=e[:, 0], hp=e[:, 1],
                 max_hp=e[:, 2])


def cmd_gen(args):
    net, dev = C.net_and_dev(args.ckpt, args.weights)
    rng = np.random.default_rng(args.seed)
    recs = train_starts([float(x) for x in args.mix.split(",")], args.count, rng)
    player = C.SearchPlayer(args.k, args.m, args.pmin, args.pskip, args.margin, args.hp_w,
                            args.pot_w, C.ROOMS, args.seed, args.loss_w)
    rec = Recorder(args.out, args.keep, args.chunk, args.seed)
    # rollout ids are global across `gen` calls into one DIR: offset by the seed
    off = args.seed * 10_000_000
    t0 = time.time()
    res = C.play(net, dev, recs, 1, args.seed, args.fights, player, args.threads,
                 on_step=lambda env, idx, k_of, lg, a: rec.step(player, env, idx, k_of + off, lg, a),
                 on_end=lambda k, w, hp, mx: rec.end(k + off, w, hp, mx))
    rec.close(off)
    meta = dict(ckpt=str(args.ckpt), weights=args.weights, count=args.count, seed=args.seed,
                mix=args.mix, k=args.k, m=args.m, pmin=args.pmin, pskip=args.pskip,
                loss_w=args.loss_w, keep=args.keep, wall_s=time.time() - t0,
                stats=dict(player.stats), died=float(res["died"].mean()))
    (pathlib.Path(args.out) / f"meta{off:09d}.json").write_text(json.dumps(meta))
    print(f"gen: {len(recs)} fights in {meta['wall_s']:.0f}s, died {meta['died']:.3f}, "
          f"{meta['stats']}", flush=True)
    return 0


# ---------------------------------------------------------------------------------------------


def fight_value_head(d):
    import torch.nn as nn

    return nn.Sequential(nn.LayerNorm(d), nn.Linear(d, 256), nn.SiLU(), nn.Linear(256, 2))


def shrunk_q(q, u, z):
    """Candidates' Q with each one's paired gain over the greedy move (candidate 0, same
    sampled futures) shrunk toward 0 by `z` standard errors (a near-tie is a tie)."""
    if u is None or z <= 0:
        return q
    u = u.astype(np.float64)
    d = u - u[:, :1]
    m = d.shape[2]
    mean = d.mean(2)
    se = d.std(2, ddof=1) / np.sqrt(m)
    gain = np.sign(mean) * np.maximum(0.0, np.abs(mean) - z * se)
    return np.where(np.isfinite(q), q[:, :1] + gain, q)


def load_data(dirs, tau, sig=0.0, want_priv=True):
    """Rows (obs, masks), target top-8 (actions, probs), value targets (won, hp frac). The arrays
    are allocated once (a first pass counts the rows of finished fights; npz keys load lazily), so
    the peak is the data itself. `want_priv` False skips the value head's privileged input."""
    ends = {}
    parts = []
    for d in dirs:
        d = pathlib.Path(d)
        for f in sorted(d.glob("ends*.npz")):
            e = np.load(f)
            for k, w, hp, mx in zip(e["fight"], e["won"], e["hp"], e["max_hp"]):
                ends[int(k)] = (int(w), hp / max(int(mx), 1))
        parts += sorted(d.glob("part*.npz"))
    oks, n, has_priv = [], 0, want_priv
    for f in parts:
        z = np.load(f)
        ok = np.array([int(k) in ends for k in z["fight"]], dtype=bool)
        oks.append(ok)
        n += int(ok.sum())
        has_priv = has_priv and "priv" in z.files
    z = np.load(parts[0])
    obs = np.empty((n, z["obs"].shape[1]), z["obs"].dtype)
    masks = np.empty((n, z["masks"].shape[1]), z["masks"].dtype)
    topn = z["top_a"].shape[1]
    ta = np.empty((n, topn), np.int64)
    tp = np.empty((n, topn), np.float32)
    won = np.empty(n, np.float32)
    hpf = np.empty(n, np.float32)
    srch = np.empty(n, bool)
    priv = np.empty((n, z["priv"].shape[1]), z["priv"].dtype) if has_priv else None
    at = 0
    for f, ok in zip(parts, oks):
        z = np.load(f)
        m = int(ok.sum())
        if not m:
            continue
        a = z["top_a"][ok].astype(np.int64)
        p = z["top_p"][ok].astype(np.float64)
        p = p / p.sum(1, keepdims=True)
        cand = z["cand"][ok]
        q = shrunk_q(z["q"][ok].astype(np.float64), z["u"][ok] if "u" in z else None, sig)
        s = z["searched"][ok]
        for r in np.nonzero(s)[0]:
            cs = [(int(c), qq) for c, qq in zip(cand[r], q[r]) if c >= 0 and np.isfinite(qq)]
            pos = {int(x): j for j, x in enumerate(a[r])}
            js = [pos[c] for c, _ in cs if c in pos]
            if len(js) < 2:
                continue
            qs = np.array([qq for c, qq in cs if c in pos])
            mass = p[r, js].sum()
            logit = np.log(np.maximum(p[r, js], 1e-9)) + qs / tau
            w = np.exp(logit - logit.max())
            p[r, js] = mass * w / w.sum()
        sl = slice(at, at + m)
        obs[sl] = z["obs"][ok]
        masks[sl] = z["masks"][ok]
        if priv is not None:  # CB1 MCTS data: the value head's privileged input
            priv[sl] = z["priv"][ok]
        ta[sl] = a
        tp[sl] = p
        e = np.array([ends[int(k)] for k in z["fight"][ok]], dtype=np.float32).reshape(-1, 2)
        won[sl] = e[:, 0]
        hpf[sl] = e[:, 1]
        srch[sl] = s
        at += m
    return obs, masks, ta, tp, won, hpf, srch, priv


def cmd_train(args):
    import torch
    import torch.nn.functional as F

    import rln36_boss as B
    from sts2.e2e import encode as E

    torch.manual_seed(args.seed)
    net, dev = C.net_and_dev(args.ckpt, args.weights)
    state = torch.load(args.ckpt, map_location="cpu", weights_only=False)
    f = B.flags(net)
    d = net.norm.weight.shape[0]
    vh = fight_value_head(d).to(dev)
    if "naz3_value" in state:
        vh.load_state_dict(state["naz3_value"])
    obs, masks, ta, tp, won, hpf, srch, priv = load_data(args.data, args.tau, args.z,
                                                         want_priv=args.pv > 0)
    use_pv = priv is not None and args.pv > 0
    # CB1: the PPO value head's target, the gym return from the row on (win bonus 1 plus
    # hp_coef x the HP fraction still to change; the fraction is 0 after a loss)
    phi0 = (np.maximum(0, C.i16_col(obs, C.H_HP)) / np.maximum(1, C.i16_col(obs, C.H_MAXHP)))
    ret = (won + args.hp_coef * (np.where(won > 0, hpf, 0.0) - phi0)).astype(np.float32)
    n = len(obs)
    rng = np.random.default_rng(args.seed)
    val = rng.random(n) < args.val
    tr = np.nonzero(~val)[0]
    vi = np.nonzero(val)[0]
    print(f"train: {len(tr)} rows ({srch[tr].mean():.2f} searched), val {len(vi)}, "
          f"won {won.mean():.3f}", flush=True)
    net.train()
    params = list(net.policy_parameters()) + list(vh.parameters())
    if use_pv:
        seen = {id(q) for q in params}
        params += [q for q in net.value_parameters() if id(q) not in seen]
    opt = torch.optim.AdamW(params, lr=args.lr, weight_decay=1e-4)
    steps = args.epochs * (len(tr) // args.batch)
    sched = torch.optim.lr_scheduler.LambdaLR(
        opt, lambda s: min(1.0, (s + 1) / 100) * max(0.0, 1 - s / max(steps, 1)))

    def batch(rows):
        o = torch.from_numpy(obs[rows]).to(dev)
        mk = torch.from_numpy(masks[rows].view(np.int64)).to(dev)
        tok = E.encode(o, mk, **f)
        legal = E.mask_bits(mk, discard=f["discard"])
        with torch.autocast(dev.type, dtype=torch.bfloat16, enabled=dev.type == "cuda"):
            lg, h, valid = net.policy(tok)
            v = vh(h[:, 0]).float()
            if use_pv:
                ptok = E.encode_priv(torch.from_numpy(priv[rows]).to(dev))
                vp = net.value(h, valid, ptok, tok["combat"]).float()
        lg = lg.float()[:, :legal.shape[1]].masked_fill(~legal[:, :lg.shape[1]], -1e9)
        logp = torch.log_softmax(lg, -1)
        a = torch.from_numpy(ta[rows]).to(dev)
        p = torch.from_numpy(tp[rows]).to(dev)
        pol = -(p * logp.gather(1, a)).sum(1)
        w = torch.from_numpy(won[rows]).to(dev)
        hf = torch.from_numpy(hpf[rows]).to(dev)
        vl = F.binary_cross_entropy_with_logits(v[:, 0], w) + (w * (torch.sigmoid(v[:, 1]) - hf) ** 2).sum() / w.sum().clamp(min=1)
        if use_pv:
            vl = vl + args.pv * F.mse_loss(vp, torch.from_numpy(ret[rows]).to(dev))
        agree = (logp.argmax(1) == a[torch.arange(len(rows)), p.argmax(1)]).float()
        return pol, vl, agree

    def evaluate():
        net.eval()
        tot = np.zeros(4)
        with torch.no_grad():
            for k in range(0, len(vi), args.batch):
                rows = vi[k:k + args.batch]
                pol, vl, ag = batch(rows)
                s = torch.from_numpy(srch[rows]).to(dev)
                tot += [float(pol.sum()), float(vl) * len(rows), float(ag.sum()),
                        float((ag * s).sum()) / max(1, float(s.sum())) * len(rows)]
        net.train()
        return tot / max(1, len(vi))

    print(f"before: val CE {evaluate()[0]:.4f}", flush=True)
    t0, step = time.time(), 0
    for ep in range(args.epochs):
        order = rng.permutation(tr)
        for k in range(0, len(order) - args.batch + 1, args.batch):
            rows = order[k:k + args.batch]
            pol, vl, _ = batch(rows)
            loss = pol.mean() + args.vf * vl
            opt.zero_grad(set_to_none=True)
            loss.backward()
            torch.nn.utils.clip_grad_norm_(params, 1.0)
            opt.step()
            sched.step()
            step += 1
            if step % 200 == 0:
                print(f"  ep {ep} step {step}/{steps}: pol {float(pol.mean()):.4f} "
                      f"v {float(vl):.4f} {time.time() - t0:.0f}s", flush=True)
        ev = evaluate()
        print(f"epoch {ep}: val CE {ev[0]:.4f} value {ev[1]:.4f} top-1 agree {ev[2]:.3f} "
              f"(searched rows {ev[3]:.3f})",
              flush=True)
    out = dict(state)
    out["net"] = net.state_dict()
    out["emas"] = []
    out["naz3_value"] = vh.state_dict()
    out["naz3"] = dict(src=str(args.ckpt), weights=args.weights, data=args.data, lr=args.lr,
                       epochs=args.epochs, tau=args.tau, vf=args.vf, z=args.z, rows=int(n))
    out.pop("opt", None)
    for k in [k for k in out if "optim" in k]:
        out.pop(k)
    tmp = pathlib.Path(str(args.out) + ".tmp")
    torch.save(out, tmp)
    tmp.rename(args.out)
    print(f"wrote {args.out} in {time.time() - t0:.0f}s", flush=True)
    return 0


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    g = sub.add_parser("gen")
    g.add_argument("ckpt")
    g.add_argument("--weights", default="ema0.999")
    g.add_argument("--count", type=int, required=True)
    g.add_argument("--seed", type=int, required=True)
    g.add_argument("--mix", default="0.4,0.35,0.25", help="boss, elite, hallway shares")
    g.add_argument("--fights", type=int, default=768)
    g.add_argument("--threads", type=int, default=12)
    g.add_argument("--k", type=int, default=3)
    g.add_argument("--m", type=int, default=8)
    g.add_argument("--pmin", type=float, default=0.05)
    g.add_argument("--pskip", type=float, default=0.95)
    g.add_argument("--margin", type=float, default=0.0)
    g.add_argument("--hp-w", type=float, default=1.0)
    g.add_argument("--pot-w", type=float, default=0.05)
    g.add_argument("--loss-w", type=float, default=0.5)
    g.add_argument("--keep", type=float, default=0.3, help="share of unsearched rows stored")
    g.add_argument("--chunk", type=int, default=100_000)
    g.add_argument("--out", required=True)
    t = sub.add_parser("train")
    t.add_argument("ckpt")
    t.add_argument("--weights", default="ema0.999")
    t.add_argument("--data", nargs="+", required=True)
    t.add_argument("--out", required=True)
    t.add_argument("--epochs", type=int, default=2)
    t.add_argument("--lr", type=float, default=3e-5)
    t.add_argument("--batch", type=int, default=512)
    t.add_argument("--tau", type=float, default=0.03)
    t.add_argument("--vf", type=float, default=0.25)
    t.add_argument("--z", type=float, default=0.0,
                   help="shrink each candidate's paired gain by z standard errors (needs `u`)")
    t.add_argument("--val", type=float, default=0.03)
    t.add_argument("--pv", type=float, default=1.0,
                   help="CB1: weight of the PPO value head's MSE to the row's return (needs priv)")
    t.add_argument("--hp-coef", type=float, default=0.3)
    t.add_argument("--seed", type=int, default=0)
    args = p.parse_args()
    return {"gen": cmd_gen, "train": cmd_train}[args.cmd](args)


if __name__ == "__main__":
    raise SystemExit(main())
