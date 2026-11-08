"""RLB17: fit the value net to the value search's own play (`search-collect` directories).

Over VN1's `vn1_train.py` (outcomes only): the search's root value `q` (its best first action's
total per world: P(win) + 0.3 × HP kept, mixed with a playout, after one more turn of search)
is a second target, on the leaf scale s = P(win) + 0.3 × min(HP kept, HP now):
loss = BCE(win, z) + BCE(hp, hp_z) + `--q` × (s − q)². Rows are the search's turn-start
decisions (the positions its leaves are), or every decision with `--all`.

    .venv/bin/python python/rlb17_value.py --data D1 D2 --init RLB11/run2/last.pt --out DIR
"""

import argparse
import pathlib

import numpy as np
import torch
import torch.nn.functional as F

import vn1_common as vc
from sts2 import combat_net

HP_W = 0.3


def load(dirs, all_rows):
    raws, ts = [], []
    for d in dirs:
        d = pathlib.Path(d)
        t = np.loadtxt(d / "samples.tsv", dtype=np.int64, ndmin=2)
        o = np.fromfile(d / "obs.bin", np.uint8)
        n = min(len(o) // vc.W, len(t))
        raw, t = o[: n * vc.W].reshape(n, vc.W), t[:n]
        keep = np.ones(n, bool) if all_rows else t[:, 7] == 1
        raws.append(raw[keep])
        ts.append(t[keep])
    return np.concatenate(raws), np.concatenate(ts)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--data", nargs="+", required=True)
    ap.add_argument("--init", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--all", action="store_true")
    ap.add_argument("--val-from", type=int, default=1021800)
    ap.add_argument("--q", type=float, default=10.0, help="weight of the search-value target")
    ap.add_argument("--z", type=float, default=1.0, help="weight of the outcome targets")
    ap.add_argument("--epochs", type=int, default=6)
    ap.add_argument("--lr", type=float, default=5e-5)
    ap.add_argument("--minibatch", type=int, default=2048)
    ap.add_argument("--kl", type=float, default=1.0)
    ap.add_argument("--wd", type=float, default=0.0)
    ap.add_argument("--seed", type=int, default=0)
    ap.add_argument("--device", default="cuda")
    a = ap.parse_args()
    torch.manual_seed(a.seed)
    torch.set_num_threads(2)
    dev = torch.device(a.device)
    out = pathlib.Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    start, saved = vc.load_net(a.init, dev)
    net, _ = vc.load_net(a.init, dev)
    raw, t = load(a.data, a.all)
    win_np, hp_np = vc.targets(t)
    q_np = np.clip(t[:, 8] / 1e6, 0.0, 1.0 + HP_W).astype(np.float32)
    held = t[:, 0] >= a.val_from
    x = torch.from_numpy(raw).to(dev)
    win, hp, q = (torch.from_numpy(v).to(dev) for v in (win_np, hp_np, q_np))
    hp_now = torch.from_numpy((np.clip(raw[:, 0].astype(np.int64) + raw[:, 1].astype(np.int64) * 256, 0, None)
                               / np.maximum(raw[:, 2].astype(np.int64) + raw[:, 3].astype(np.int64) * 256, 1)
                               ).astype(np.float32)).to(dev)
    enc = raw[:, combat_net.OBS_BYTES + 4].astype(np.int64) + raw[:, combat_net.OBS_BYTES + 5].astype(np.int64) * 256
    turn = t[:, 6]
    pol0, _, _ = vc.heads(start, x)
    tr = torch.nonzero(torch.from_numpy(~held)).squeeze(1).to(dev)
    ev = torch.from_numpy(held).to(dev)
    print(f"{len(x)} rows, {len(tr)} train, {int(ev.sum())} held out; won {win_np.mean():.3f} q {q_np.mean():.3f}",
          flush=True)

    def leaf(wl, hl, now):
        return torch.sigmoid(wl) + HP_W * torch.minimum(torch.sigmoid(hl), now)

    def report(tag):
        net.eval()
        _, wl, hl = vc.heads(net, x[ev])
        s = leaf(wl, hl, hp_now[ev]).cpu().numpy()
        p = torch.sigmoid(wl).cpu().numpy()
        w, qq = win_np[held], q_np[held]
        bce = -np.mean(w * np.log(np.clip(p, 1e-6, 1)) + (1 - w) * np.log(np.clip(1 - p, 1e-6, 1)))
        st = enc[held] * 100 + np.minimum(turn[held], 12)
        num = den = 0.0
        for key in np.unique(st):
            m = st == key
            pos, neg = w[m].sum(), (1 - w[m]).sum()
            if pos and neg:
                num += vc.auc(s[m], w[m]) * pos * neg
                den += pos * neg
        print(f"{tag}: bce {bce:.4f} pred {p.mean():.3f} won {w.mean():.3f} | q mse {np.mean((s - qq) ** 2):.5f} "
              f"corr {np.corrcoef(s, qq)[0, 1]:.4f} | strat-auc s {num / max(den, 1e-9):.4f}", flush=True)

    st_all = enc[held] * 100 + np.minimum(turn[held], 12)
    num = den = 0.0
    for key in np.unique(st_all):
        m = st_all == key
        pos, neg = win_np[held][m].sum(), (1 - win_np[held][m]).sum()
        if pos and neg:
            num += vc.auc(q_np[held][m], win_np[held][m]) * pos * neg
            den += pos * neg
    print(f"held-out strat-auc of the search's q: {num / max(den, 1e-9):.4f}", flush=True)
    report("start")
    opt = torch.optim.AdamW(net.parameters(), lr=a.lr, eps=1e-5, weight_decay=a.wd)
    steps = a.epochs * ((len(tr) + a.minibatch - 1) // a.minibatch)
    sched = torch.optim.lr_scheduler.OneCycleLR(opt, a.lr, total_steps=steps, pct_start=0.1)
    for epoch in range(1, a.epochs + 1):
        net.train()
        acc = []
        for chunk in tr[torch.randperm(len(tr), device=dev)].split(a.minibatch):
            logits, wl, hl = net(x[chunk])
            zl = (F.binary_cross_entropy_with_logits(wl, win[chunk])
                  + F.binary_cross_entropy_with_logits(hl, hp[chunk]))
            ql = ((leaf(wl, hl, hp_now[chunk]) - q[chunk]) ** 2).mean()
            kl = F.kl_div(F.log_softmax(logits, 1), F.log_softmax(pol0[chunk], 1),
                          log_target=True, reduction="batchmean")
            loss = a.z * zl + a.q * ql + a.kl * kl
            opt.zero_grad(set_to_none=True)
            loss.backward()
            torch.nn.utils.clip_grad_norm_(net.parameters(), 1.0)
            opt.step()
            sched.step()
            acc.append([zl.item(), ql.item(), kl.item()])
        m = np.mean(acc, axis=0)
        print(f"epoch {epoch} z {m[0]:.4f} q {m[1]:.5f} kl {m[2]:.4f}", flush=True)
        report(f"epoch {epoch}")
        combat_net.export(net, out / f"e{epoch}.bin", saved["vocab_hash"])
    torch.save({**saved, "model": net.state_dict(), "opt": None, "rlb17_args": vars(a)}, out / "last.pt")


if __name__ == "__main__":
    main()
