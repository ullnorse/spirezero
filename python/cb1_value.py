"""CB1 round 2: a supervised value net for the MCTS leaves.

    python python/cb1_value.py gen CKPT --count N --seed S --out DIR   # labelled positions
    python python/cb1_value.py train CKPT --data DIR... --out F.pt     # fine-tune trunk + value

**Labels.** Training starts (`train_b`, bench and panel ids excluded) are played greedily by the
net; at a share `--p` of its decisions the position is recorded and `--k` reseeded copies of it
(`Sts2RunEnvs.copy(reseed)`: the hidden state resampled) are played to the fight's end by the
net, greedily. The label is the PPO value head's quantity, so the result is a drop-in leaf value
(`cb1_mcts.py --value-ckpt`): the mean over copies of `won + hp_coef * phi_end`, minus
`hp_coef * phi_now` (phi = HP / max HP, 0 after a death). The mean over resampled worlds is the
information set's value under the net's play, with far less noise than one outcome. The fight's
own outcome is stored too (`ends*.npz`).

**Net.** A copy of CKPT whose trunk and value head are fine-tuned by MSE to the labels, on the
recorded position's own privileged input (a sample of the information set, as a reseeded world's
is at an MCTS leaf). Its policy head is not trained and not used.
"""

import argparse
import collections
import json
import pathlib
import sys
import time

import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import naz3_combat as C  # noqa: E402

HP_COEF = 0.3


class LabelPlayer:
    """Greedy player that labels a share of its decisions by `k` reseeded greedy rollouts."""

    def __init__(self, k, p, seed):
        import cb1_mcts as M

        self.k, self.p = k, p
        self.rng = np.random.default_rng(seed)
        self.env_kw = dict(priv=True)
        self.rooms = set(C.ROOMS)
        self.stats = collections.Counter()
        self.base = 0
        self.roll = M.MCTSPlayer(leaf="full", pot_w=0.0, hp_coef=HP_COEF)
        self.rows = collections.defaultdict(list)

    def scratch(self, fights):
        return fights * self.k

    def choose(self, net, env, dev, mains, lg, room_ok):
        from sts2 import HOLD

        import cb1_mcts as M

        x = lg.float().cpu().numpy().astype(np.float64)
        n = len(mains)
        self.last_searched = np.zeros(n, dtype=np.int32)
        self.last_rows = np.zeros(n, dtype=np.int64)
        greedy = x.argmax(1)
        dead = ~np.isfinite(x).any(1)
        self.stats["decisions"] += n
        if dead.any():  # same rule as the MCTS player: the next engine step must be the real one
            self.stats["skipped_steps"] += 1
            return np.where(dead, HOLD, greedy)
        pick = np.nonzero(self.rng.random(n) < self.p)[0]
        if not len(pick):
            return greedy
        mains = np.asarray(mains)
        phi0 = M.phi(env.obs[mains[pick]])
        if self.k == 0:  # labelled later by the fight's own outcome (`ends*.npz`)
            self.stats["labelled"] += len(pick)
            self.cur = dict(rows=pick, obs=env.obs[mains[pick]].copy(),
                            masks=env.masks[mains[pick]].copy(), priv=env.priv[mains[pick]].copy(),
                            label=np.full(len(pick), np.nan, dtype=np.float32),
                            sd=np.zeros(len(pick)), phi0=phi0.astype(np.float32))
            return greedy
        src = np.repeat(mains[pick], self.k)
        dst = self.base + (src * self.k + np.tile(np.arange(self.k), len(pick)))
        env.copy(src, dst, reseed=self.rng.integers(0, 2**63, size=len(src), dtype=np.uint64))
        lgd, _ = M.eval_net(net, env, dst, dev, value=False)
        u = self.roll.rollout(net, env, dev, dst, lgd, np.zeros(len(dst)), HOLD)
        self.stats["labelled"] += len(pick)
        self.stats["rollouts"] += len(dst)
        lab = u.reshape(len(pick), self.k).mean(1) - HP_COEF * phi0
        self.cur = dict(rows=pick, obs=env.obs[mains[pick]].copy(),
                        masks=env.masks[mains[pick]].copy(), priv=env.priv[mains[pick]].copy(),
                        label=lab.astype(np.float32), sd=u.reshape(len(pick), self.k).std(1),
                        phi0=phi0.astype(np.float32))
        return greedy


def starts(path, mix, count, rng, exclude):
    """`cb1_gumbel.starts` (same draws), streaming the file: only each line's offset and room are
    kept, then the chosen lines are read (the start files are too big to hold parsed)."""
    by = {room: [] for room in C.ROOMS}
    with open(path, "rb") as fh:
        off = 0
        for line in fh:
            r = json.loads(line)
            if r.get("id") not in exclude:
                by[C.room_of(r)].append(off)
            off += len(line)
    out = []
    for room, share in zip(("boss", "elite", "hallway"), mix):
        n = int(round(count * share))
        if n and by[room]:
            out += [by[room][i] for i in rng.choice(len(by[room]), n, replace=len(by[room]) < n)]
    out = [out[i] for i in rng.permutation(len(out))]
    recs = []
    with open(path, "rb") as fh:
        for o in out:
            fh.seek(o)
            recs.append(json.loads(fh.readline()))
    return recs


def cmd_gen(a):
    net, dev = C.net_and_dev(a.ckpt, a.weights)
    rng = np.random.default_rng(a.seed)
    excl = {r["id"] for r in C.read_jsonl(C.DATA / "bench.jsonl")}
    excl |= {r["id"] for r in C.read_jsonl(pathlib.Path(a.panel).expanduser())}
    recs = starts(pathlib.Path(a.starts).expanduser(), [float(x) for x in a.mix.split(",")],
                  a.count, rng, excl)
    player = LabelPlayer(a.k, a.p, a.seed)
    out = pathlib.Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    buf = collections.defaultdict(list)
    won = {}
    off = a.seed * 10**7  # fight ids unique across seeds (train holds out whole fights)
    parts, rows, lab = [0], [0], []

    def flush():
        if not buf["label"]:
            return
        z = {k: np.concatenate(v) for k, v in buf.items()}
        np.savez(out / f"vals{a.seed:04d}-{parts[0]:03d}.npz", **z)
        lab.append(z["label"])
        parts[0] += 1
        buf.clear()

    def on_step(env, idx, k_of, lg, act):
        cur = getattr(player, "cur", None)
        if cur is None:
            return
        player.cur = None
        for key in ("obs", "masks", "priv", "label", "sd", "phi0"):
            buf[key].append(cur[key])
        ks = k_of[cur["rows"]]
        buf["fight"].append(ks.astype(np.int64) + off)
        buf["act"].append(np.array([recs[k]["act"] for k in ks], dtype=np.int8))
        buf["room"].append(np.array([C.room_of(recs[k]) for k in ks]))
        rows[0] += len(ks)
        if rows[0] >= a.chunk:
            rows[0] = 0
            flush()

    def on_end(k, w, hp, mx):
        won[int(k) + off] = (w, hp / max(mx, 1))

    t0 = time.time()
    C.play(net, dev, recs, 1, a.seed, a.fights, player, a.threads, on_step=on_step, on_end=on_end)
    flush()
    ks = np.array(sorted(won), dtype=np.int64)
    np.savez(out / f"ends{a.seed:04d}.npz", fight=ks,
             won=np.array([won[k][0] for k in ks], dtype=np.float32),
             hpf=np.array([won[k][1] for k in ks], dtype=np.float32))
    allab = np.concatenate(lab)
    meta = dict(ckpt=str(a.ckpt), count=a.count, k=a.k, p=a.p, seed=a.seed, rows=len(allab),
                wall_s=time.time() - t0, stats=dict(player.stats), label_mean=float(np.nanmean(allab)) if np.isfinite(allab).any() else None)
    (out / f"meta{a.seed:04d}.json").write_text(json.dumps(meta))
    print(json.dumps(meta), flush=True)
    return 0


def cmd_train(a):
    import torch
    import torch.nn.functional as F

    import rln36_boss as B
    from sts2.e2e import encode as E

    torch.manual_seed(a.seed)
    net, dev = C.net_and_dev(a.ckpt, a.weights)
    state = torch.load(a.ckpt, map_location="cpu", weights_only=False)
    f = B.flags(net)
    parts = [np.load(p) for d in a.data for p in sorted(pathlib.Path(d).expanduser().glob("vals*.npz"))]
    sizes = [len(z["label"]) for z in parts]

    def cat(k):  # preallocated: a concatenate would hold the big fields twice
        first = parts[0][k]
        out = np.empty((sum(sizes),) + first.shape[1:], dtype=first.dtype)
        at = 0
        for z, n in zip(parts, sizes):
            out[at:at + n] = first if at == 0 else z[k]
            at += n
        return out
    obs, masks, priv, lab = cat("obs"), cat("masks"), cat("priv"), cat("label")
    fight = cat("fight")
    miss = np.isnan(lab)
    if miss.any():  # --k 0 rows: the fight's own end, `won + hp_coef * hpf - hp_coef * phi0`
        util = {}
        for d in a.data:
            for e in pathlib.Path(d).expanduser().glob("ends*.npz"):
                z = np.load(e)
                util.update(zip(z["fight"].tolist(), (z["won"] * (1 + HP_COEF * z["hpf"])).tolist()))
        phi0 = cat("phi0")
        lab = lab.copy()
        lab[miss] = [util[int(k)] - HP_COEF * p for k, p in zip(fight[miss], phi0[miss])]
    n = len(lab)
    rng = np.random.default_rng(a.seed)
    uf = np.unique(fight)
    vf = set(rng.choice(uf, max(1, int(len(uf) * a.val)), replace=False).tolist())
    val = np.array([x in vf for x in fight])  # held out by fight: no leak between positions
    tr, vi = np.nonzero(~val)[0], np.nonzero(val)[0]
    print(f"train: {len(tr)} rows, val {len(vi)}; label mean {lab.mean():.3f} sd {lab.std():.3f}",
          flush=True)
    net.train()
    params = list(net.value_parameters() if a.head_only else net.parameters())
    opt = torch.optim.AdamW(params, lr=a.lr, weight_decay=1e-4)
    steps = a.epochs * (len(tr) // a.batch)
    sched = torch.optim.lr_scheduler.LambdaLR(
        opt, lambda s: min(1.0, (s + 1) / 200) * max(0.0, 1 - s / max(steps, 1)))

    def pred(rows):
        o = torch.from_numpy(obs[rows]).to(dev)
        mk = torch.from_numpy(masks[rows].view(np.int64)).to(dev)
        tok = E.encode(o, mk, **f)
        with torch.autocast(dev.type, dtype=torch.bfloat16, enabled=dev.type == "cuda"):
            _, h, valid = net.policy(tok)
            ptok = E.encode_priv(torch.from_numpy(priv[rows]).to(dev))
            return net.value(h, valid, ptok, tok["combat"]).float()

    def evaluate():
        net.eval()
        se, ps = 0.0, []
        with torch.no_grad():
            for k in range(0, len(vi), a.batch):
                rows = vi[k:k + a.batch]
                p = pred(rows).cpu().numpy()
                ps.append(p)
                se += float(((p - lab[rows]) ** 2).sum())
        net.train()
        p = np.concatenate(ps)
        return se / max(1, len(vi)), float(np.corrcoef(p, lab[vi])[0, 1])

    print(f"before: val MSE {evaluate()}", flush=True)
    t0, step = time.time(), 0
    for ep in range(a.epochs):
        order = rng.permutation(tr)
        for k in range(0, len(order) - a.batch + 1, a.batch):
            rows = order[k:k + a.batch]
            loss = F.mse_loss(pred(rows), torch.from_numpy(lab[rows]).to(dev))
            opt.zero_grad(set_to_none=True)
            loss.backward()
            torch.nn.utils.clip_grad_norm_(params, 1.0)
            opt.step()
            sched.step()
            step += 1
            if step % 500 == 0:
                print(f"  ep {ep} step {step}/{steps}: mse {float(loss):.4f} {time.time() - t0:.0f}s",
                      flush=True)
        print(f"epoch {ep}: val (MSE, corr) {evaluate()}", flush=True)
    out = dict(state)
    out["net"] = net.state_dict()
    out["emas"] = []
    out["cb1_value"] = dict(src=str(a.ckpt), data=a.data, lr=a.lr, epochs=a.epochs, rows=int(n))
    for k in [k for k in out if k == "opt" or "optim" in k]:
        out.pop(k)
    tmp = pathlib.Path(str(a.out) + ".tmp")
    torch.save(out, tmp)
    tmp.rename(a.out)
    print(f"wrote {a.out} in {time.time() - t0:.0f}s", flush=True)
    return 0


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    g = sub.add_parser("gen")
    g.add_argument("ckpt")
    g.add_argument("--weights", default="raw")
    g.add_argument("--starts", default="~/.cache/spirebot-data/cb1/train_b.jsonl")
    g.add_argument("--panel", default="~/.cache/spirebot-data/cb1/panel_bench.jsonl")
    g.add_argument("--count", type=int, required=True)
    g.add_argument("--mix", default="0.5,0.4,0.1", help="boss, elite, hallway shares")
    g.add_argument("--k", type=int, default=4, help="rollouts per label; 0: the fight's own end")
    g.add_argument("--p", type=float, default=0.25)
    g.add_argument("--seed", type=int, default=0)
    g.add_argument("--fights", type=int, default=512)
    g.add_argument("--chunk", type=int, default=100_000)
    g.add_argument("--threads", type=int, default=12)
    g.add_argument("--out", required=True)
    t = sub.add_parser("train")
    t.add_argument("ckpt")
    t.add_argument("--weights", default="raw")
    t.add_argument("--data", nargs="+", required=True)
    t.add_argument("--out", required=True)
    t.add_argument("--epochs", type=int, default=2)
    t.add_argument("--lr", type=float, default=5e-5)
    t.add_argument("--batch", type=int, default=512)
    t.add_argument("--val", type=float, default=0.03)
    t.add_argument("--seed", type=int, default=0)
    t.add_argument("--head-only", action="store_true", help="train the value head, not the trunk")
    a = p.parse_args()
    return {"gen": cmd_gen, "train": cmd_train}[a.cmd](a)


if __name__ == "__main__":
    raise SystemExit(main())
