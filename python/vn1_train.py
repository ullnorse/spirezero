"""VN1: refit the combat net's win and HP heads (and its body) to the real bot's own fight
outcomes (`value-collect`, RLB12 vc-a) and optionally the tree's (`az-collect`, RLB13 ei1-data),
for the search's leaves.

Over RLB12's `rlb12_value_ei.py`: seeds `--hold A B` are never trained on (the screen's fights
come from them), `--widen` doubles the hidden width function-preservingly before training, and
`--rank` adds a pairwise loss on boss rows: within a (boss, turn) stratum, a position whose
fight was won should score above one whose fight was lost, on the search's leaf scale
P(win) + 0.3 × HP kept. The policy head is held near the start net (KL) for the commons.

    .venv/bin/python python/vn1_train.py --data VC --init RLB11/run2/last.pt --out DIR
"""

import argparse
import pathlib

import numpy as np
import torch
import torch.nn.functional as F

import vn1_common as vc
from sts2 import combat_net

HP_W = 0.3


def widen(net, saved, device, noise=1e-3):
    """`net` at twice the hidden width, computing the same function (plus `noise`)."""
    h = saved["hidden"]
    big = combat_net.Net(2 * h).to(device)
    sd, new = net.state_dict(), {}
    for k, v in sd.items():
        if k.startswith("emb."):
            new[k] = v
    new["l1.weight"] = torch.cat([sd["l1.weight"]] * 2)
    new["l1.bias"] = torch.cat([sd["l1.bias"]] * 2)
    w2 = sd["l2.weight"] / 2
    new["l2.weight"] = torch.cat([torch.cat([w2, w2], 1)] * 2)
    new["l2.bias"] = torch.cat([sd["l2.bias"]] * 2)
    for head in ("policy", "win", "hp"):
        w = sd[f"{head}.weight"] / 2
        new[f"{head}.weight"] = torch.cat([w, w], 1)
        new[f"{head}.bias"] = sd[f"{head}.bias"]
    for k in ("l1.weight", "l2.weight"):
        new[k] = new[k] + noise * torch.randn_like(new[k]) * new[k].std()
    big.load_state_dict(new)
    return big


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--data", required=True)
    ap.add_argument("--tree", default=None, help="az-collect data, boss rows")
    ap.add_argument("--tree-weight", type=float, default=1.0)
    ap.add_argument("--init", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--hold", type=int, nargs=2, default=[1022000, 1022450])
    ap.add_argument("--elite", type=float, default=3.0)
    ap.add_argument("--boss", type=float, default=6.0)
    ap.add_argument("--epochs", type=int, default=8)
    ap.add_argument("--lr", type=float, default=5e-5)
    ap.add_argument("--minibatch", type=int, default=4096)
    ap.add_argument("--kl", type=float, default=1.0)
    ap.add_argument("--rank", type=float, default=0.0)
    ap.add_argument("--rank-scale", type=float, default=10.0)
    ap.add_argument("--widen", action="store_true")
    ap.add_argument("--wd", type=float, default=0.0)
    ap.add_argument("--td-teacher", default=None,
                    help="a net whose next-turn values bootstrap the targets (TD(λ) along each fight)")
    ap.add_argument("--lam", type=float, default=0.8)
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
    if a.widen:
        net = widen(net, saved, dev)
    hidden = net.l1.weight.shape[0]

    raw, t, fight, enc, turn = vc.load(a.data)
    src = np.zeros(len(t), np.int64)
    if a.tree:
        r2, t2, f2, e2, u2 = vc.load(a.tree)
        raw, t = np.concatenate([raw, r2]), np.concatenate([t, t2])
        fight = np.concatenate([fight, f2 + fight.max() + 1])
        enc, turn = np.concatenate([enc, e2]), np.concatenate([turn, u2])
        src = np.concatenate([src, np.ones(len(t2), np.int64)])
    win_np, hp_np = vc.targets(t)
    held = (t[:, 0] >= a.hold[0]) & (t[:, 0] < a.hold[1])
    ev_np = held & (src == 0)
    x = torch.from_numpy(raw).to(dev)
    kind = torch.from_numpy(t[:, 1]).to(dev)
    win, hp = torch.from_numpy(win_np).to(dev), torch.from_numpy(hp_np).to(dev)
    hp_now = torch.from_numpy((np.clip(raw[:, 0].astype(np.int64) + raw[:, 1].astype(np.int64) * 256, 0, None)
                               / np.maximum(raw[:, 2].astype(np.int64) + raw[:, 3].astype(np.int64) * 256, 1)
                               ).astype(np.float32)).to(dev)
    stratum = torch.from_numpy(enc * 100 + np.minimum(turn, 12)).to(dev)
    weight = torch.ones_like(win)
    weight[kind == 1] = a.elite
    weight[kind == 2] = a.boss
    weight[torch.from_numpy(src == 1).to(dev)] *= a.tree_weight
    pol0, _, _ = vc.heads(start, x)
    if a.td_teacher:
        teacher, _ = vc.load_net(a.td_teacher, dev)
        _, tw, th = vc.heads(teacher, x)
        tw, th = torch.sigmoid(tw).cpu().numpy(), torch.sigmoid(th).cpu().numpy()
        gw, gh = win_np.copy(), hp_np.copy()
        for i in range(len(t) - 2, -1, -1):  # λ-returns, backwards within each fight
            if fight[i + 1] == fight[i]:
                gw[i] = (1 - a.lam) * tw[i + 1] + a.lam * gw[i + 1]
                gh[i] = (1 - a.lam) * th[i + 1] + a.lam * gh[i + 1]
        win_t, hp_t = torch.from_numpy(gw).to(dev), torch.from_numpy(gh).to(dev)
        del teacher
    else:
        win_t, hp_t = win, hp
    tr = torch.nonzero(torch.from_numpy(~held)).squeeze(1).to(dev)
    boss_tr = tr[kind[tr] == 2]
    ev = torch.from_numpy(ev_np).to(dev)
    print(f"{len(x)} rows ({int((src == 1).sum())} tree), {len(tr)} train ({len(boss_tr)} boss), "
          f"{int(ev.sum())} held out; hidden {hidden}", flush=True)
    enc_ev, turn_ev = enc[ev_np], turn[ev_np]
    names = vc.encounters()

    def report(tag):
        net.eval()
        _, wl, hl = vc.heads(net, x[ev])
        p, q = torch.sigmoid(wl).cpu().numpy(), torch.sigmoid(hl).cpu().numpy()
        w, h, k = win[ev].cpu().numpy(), hp[ev].cpu().numpy(), kind[ev].cpu().numpy()
        s = p + HP_W * np.minimum(q, hp_now[ev].cpu().numpy())
        parts = []
        for kk, nm in enumerate(("common", "elite", "boss")):
            m = k == kk
            bce = -np.mean(w[m] * np.log(np.clip(p[m], 1e-6, 1)) + (1 - w[m]) * np.log(np.clip(1 - p[m], 1e-6, 1)))
            parts.append(f"{nm} bce {bce:.4f} won {w[m].mean():.3f} pred {p[m].mean():.3f}")
        b = k == 2
        st = enc_ev[b] * 100 + np.minimum(turn_ev[b], 12)
        num = den = 0.0
        for key in np.unique(st):
            m = st == key
            pos, neg = w[b][m].sum(), (1 - w[b][m]).sum()
            if pos and neg:
                num += vc.auc(s[b][m], w[b][m]) * pos * neg
                den += pos * neg
        hb = b & (w > 0)
        tops = []
        for nm in ("TEST_SUBJECT_BOSS", "QUEEN_BOSS", "AEONGLASS_BOSS"):
            e = [i for i, v in names.items() if v == nm][0]
            m = b & (np.concatenate([enc_ev]) == e)
            tops.append(f"{nm.split('_')[0][:5]} {p[m].mean():.3f}/{w[m].mean():.3f}")
        print(f"{tag}: {' | '.join(parts)} | boss strat-auc {num / den:.4f} hp {q[hb].mean():.3f}/"
              f"{h[hb].mean():.3f} | {' '.join(tops)}", flush=True)

    report("start")
    opt = torch.optim.AdamW(net.parameters(), lr=a.lr, eps=1e-5, weight_decay=a.wd)
    steps = a.epochs * ((len(tr) + a.minibatch - 1) // a.minibatch)
    sched = torch.optim.lr_scheduler.OneCycleLR(opt, a.lr, total_steps=steps, pct_start=0.1)
    for epoch in range(1, a.epochs + 1):
        net.train()
        acc = []
        for chunk in tr[torch.randperm(len(tr), device=dev)].split(a.minibatch):
            logits, wl, hl = net(x[chunk])
            wt = weight[chunk]
            vloss = ((F.binary_cross_entropy_with_logits(wl, win_t[chunk], reduction="none")
                      + F.binary_cross_entropy_with_logits(hl, hp_t[chunk], reduction="none"))
                     * wt).sum() / wt.sum()
            kl = F.kl_div(F.log_softmax(logits, 1), F.log_softmax(pol0[chunk], 1),
                          log_target=True, reduction="batchmean")
            loss = vloss + a.kl * kl
            rl = torch.zeros((), device=dev)
            if a.rank > 0:
                bi = boss_tr[torch.randint(len(boss_tr), (2048,), device=dev)]
                _, bw, bh = net(x[bi])
                s = torch.sigmoid(bw) + HP_W * torch.minimum(torch.sigmoid(bh), hp_now[bi])
                same = stratum[bi].unsqueeze(1) == stratum[bi].unsqueeze(0)
                pair = same & (win[bi].unsqueeze(1) > 0.5) & (win[bi].unsqueeze(0) < 0.5)
                if pair.any():
                    d = s.unsqueeze(1) - s.unsqueeze(0)
                    rl = F.softplus(-a.rank_scale * d[pair]).mean()
                    loss = loss + a.rank * rl
            opt.zero_grad(set_to_none=True)
            loss.backward()
            torch.nn.utils.clip_grad_norm_(net.parameters(), 1.0)
            opt.step()
            sched.step()
            acc.append([vloss.item(), kl.item(), rl.item()])
        m = np.mean(acc, axis=0)
        print(f"epoch {epoch} vloss {m[0]:.4f} kl {m[1]:.4f} rank {m[2]:.4f}", flush=True)
        report(f"epoch {epoch}")
        combat_net.export(net, out / f"e{epoch}.bin", saved["vocab_hash"])
        torch.save({**saved, "hidden": hidden, "model": net.state_dict(), "opt": None,
                    "vn1_epoch": epoch, "vn1_args": vars(a)}, out / f"e{epoch}.pt")


if __name__ == "__main__":
    main()
