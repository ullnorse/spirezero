"""RLB17: distill the value search into the playout policy (`honest/rollout.rs`).

Reads `search-collect` directories (`pol.bin`, `samples.tsv`) and fits the policy to the search's
scores per candidate: cross-entropy to softmax(score / T) plus `--hard` × cross-entropy to the
search's move. Starts from a v2 net (`--init`, a `STS2RLP2` file) or from scratch; `--hand`
adds the v4 input (RLB17): a learned vector per card key in hand, summed into the hidden layer.
Writes `STS2RLP2` (or `STS2RLP3` with `--hand`) weights that `RolloutNet::load` reads.

    .venv/bin/python python/rlb17_policy.py --data D1 D2 --init v2b.bin --hidden 64 --out OUT.bin
"""

import argparse
import pathlib
import struct

import numpy as np
import torch
import torch.nn.functional as F

F3, G, SLOTS = 84, 6, 12
INTS = 4 + 2 * SLOTS
FLOATS = F3 + 7 * SLOTS
REC = 4 * (INTS + FLOATS)
F2 = 60


def load(dirs, max_turn=60):
    ints, floats, seeds, won = [], [], [], []
    for d in dirs:
        d = pathlib.Path(d)
        raw = np.fromfile(d / "pol.bin", np.uint8)
        t = np.loadtxt(d / "samples.tsv", dtype=np.int64, ndmin=2)
        n = min(len(raw) // REC, len(t))
        raw = raw[: n * REC].reshape(n, REC)
        ints.append(raw[:, : 4 * INTS].copy().view(np.int32))
        floats.append(raw[:, 4 * INTS:].copy().view(np.float32))
        seeds.append(t[:n, 0])
        won.append(t[:n, 2])
    return np.concatenate(ints), np.concatenate(floats), np.concatenate(seeds), np.concatenate(won)


def read_v2(path):
    b = pathlib.Path(path).read_bytes()
    assert b[:8] == b"STS2RLP2", "a v2 net"
    keys, nf, g, nh = struct.unpack("<4I", b[8:24])
    vhash = b[24:32]
    x = np.frombuffer(b[32:], np.float32)
    w = x[: keys * (nf + nh)].reshape(keys, nf + nh)
    o = keys * (nf + nh)
    v = x[o:o + G]
    o += G
    a = x[o:o + nh * nf].reshape(nh, nf)
    o += nh * nf
    bb = x[o:o + nh]
    return dict(keys=keys, nf=nf, nh=nh, w=w, v=v, a=a, b=bb, hash=vhash)


class Net(torch.nn.Module):
    """`w[key] · [f, relu(A f + b + Σ e[hand])] + v · g`, as `RolloutNet`."""

    def __init__(self, keys, nf, nh, hand, init=None):
        super().__init__()
        self.nf, self.nh, self.hand = nf, nh, hand
        self.w = torch.nn.Parameter(torch.zeros(keys, nf + nh))
        self.v = torch.nn.Parameter(torch.zeros(G))
        self.a = torch.nn.Parameter((torch.rand(nh, nf) - 0.5) * 0.6)
        self.b = torch.nn.Parameter(torch.zeros(nh))
        self.e = torch.nn.Parameter(torch.zeros(keys, nh)) if hand else None
        if init is not None:
            h0 = init["nh"]
            nf0 = init["nf"]
            with torch.no_grad():
                self.w[:, :nf0] = torch.from_numpy(init["w"][:, :nf0].copy())
                self.w[:, nf:nf + h0] = torch.from_numpy(init["w"][:, nf0:].copy())
                self.v[:] = torch.from_numpy(init["v"].copy())
                self.a[:h0, :nf0] = torch.from_numpy(init["a"].copy())
                if nf > nf0:
                    self.a[:h0, nf0:] = 0
                self.b[:h0] = torch.from_numpy(init["b"].copy())
                self.w[:, nf + h0:] = 0  # new hidden units start silent

    def forward(self, f, hand, ck, cg):
        f = f[:, : self.nf]
        pre = f @ self.a.T + self.b
        if self.e is not None:
            m = (hand >= 0).float().unsqueeze(-1)
            pre = pre + (self.e[hand.clamp(min=0)] * m).sum(1)
        z = torch.cat([f, torch.relu(pre)], 1)
        wk = self.w[ck.clamp(min=0)]  # B × S × width
        return (wk * z.unsqueeze(1)).sum(-1) + cg @ self.v

    def export(self, path, vhash):
        keys = self.w.shape[0]
        magic = b"STS2RLP3" if self.e is not None else b"STS2RLP2"
        out = magic + struct.pack("<4I", keys, self.nf, G, self.nh) + vhash
        parts = [self.w, self.v, self.a, self.b] + ([self.e] if self.e is not None else [])
        x = torch.cat([p.detach().reshape(-1).float().cpu() for p in parts]).numpy()
        pathlib.Path(path).write_bytes(out + x.astype("<f4").tobytes())


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--data", nargs="+", required=True)
    ap.add_argument("--init", default=None)
    ap.add_argument("--hidden", type=int, default=64)
    ap.add_argument("--nf", type=int, default=F2, help="60 (v2 features) or 84 (+ encounter)")
    ap.add_argument("--hand", action="store_true")
    ap.add_argument("--scratch", action="store_true", help="the init's shape and vocabulary only")
    ap.add_argument("--temp", type=float, default=0.01, help="T of the soft target, in fight wins")
    ap.add_argument("--hard", type=float, default=0.5)
    ap.add_argument("--val-from", type=int, default=1021800, help="seeds from here are held out")
    ap.add_argument("--epochs", type=int, default=30)
    ap.add_argument("--lr", type=float, default=3e-3)
    ap.add_argument("--wd", type=float, default=1e-5)
    ap.add_argument("--batch", type=int, default=1024)
    ap.add_argument("--out", required=True)
    ap.add_argument("--device", default="cuda")
    a = ap.parse_args()
    torch.manual_seed(0)
    torch.set_num_threads(2)
    dev = torch.device(a.device)
    ints, floats, seeds, _ = load(a.data)
    n = ints.shape[0]
    nc = ints[:, 0]
    chosen = ints[:, 1]
    ck = ints[:, 4:4 + SLOTS].copy()
    hand = ints[:, 4 + SLOTS:].copy()
    f = floats[:, :F3]
    cg = floats[:, F3:F3 + G * SLOTS].reshape(n, SLOTS, G)
    sc = floats[:, F3 + G * SLOTS:]
    valid = (np.arange(SLOTS)[None, :] < nc[:, None]) & (sc > -8)
    keep = (valid.sum(1) >= 2)
    print(f"{n} decisions, {keep.sum()} with 2+ candidates, chosen a candidate {np.mean(chosen[keep] >= 0):.3f}",
          flush=True)
    idx = np.nonzero(keep)[0]
    tr = idx[seeds[idx] < a.val_from]
    va = idx[seeds[idx] >= a.val_from]
    T = lambda x: torch.from_numpy(np.ascontiguousarray(x)).to(dev)
    f_t, hand_t, ck_t, cg_t, sc_t = T(f), T(hand).long(), T(ck).long(), T(cg), T(sc)
    valid_t, chosen_t = T(valid), T(chosen).long()
    init = read_v2(a.init) if a.init else None
    vhash = init["hash"] if init else pathlib.Path(a.init).read_bytes()[24:32]
    keys = init["keys"]
    net = Net(keys, a.nf, a.hidden, a.hand, None if a.scratch else init).to(dev)
    opt = torch.optim.AdamW(net.parameters(), lr=a.lr, weight_decay=a.wd)
    steps = a.epochs * ((len(tr) + a.batch - 1) // a.batch)
    sched = torch.optim.lr_scheduler.OneCycleLR(opt, a.lr, total_steps=steps, pct_start=0.1)

    def batch_loss(b):
        logits = net(f_t[b], hand_t[b], ck_t[b], cg_t[b])
        m = valid_t[b]
        logits = logits.masked_fill(~m, -1e9)
        target = torch.softmax((sc_t[b] / a.temp).masked_fill(~m, -1e9), 1)
        logp = torch.log_softmax(logits, 1)
        soft = -(target * logp).sum(1).mean()
        has = chosen_t[b] >= 0
        hard = -logp[has, chosen_t[b][has]].mean() if has.any() else torch.zeros((), device=dev)
        acc = (logits.argmax(1) == chosen_t[b])[has].float().mean() if has.any() else torch.zeros((), device=dev)
        # The search's regret of the policy's argmax: its score below the best, in fight wins.
        best = sc_t[b].masked_fill(~m, -9).max(1).values
        pick = sc_t[b].gather(1, logits.argmax(1, keepdim=True)).squeeze(1)
        return soft, hard, acc, (best - pick).mean()

    def evaluate(tag):
        net.eval()
        with torch.no_grad():
            vb = torch.from_numpy(va).to(dev)
            s, h, acc, reg = [x.item() for x in batch_loss(vb)]
        net.train()
        print(f"{tag}: val soft {s:.4f} hard {h:.4f} top1 {acc:.4f} regret {reg:.5f}", flush=True)

    print(f"train {len(tr)} val {len(va)}; net nf {a.nf} hidden {a.hidden} hand {a.hand}", flush=True)
    evaluate("start")
    for ep in range(1, a.epochs + 1):
        perm = torch.from_numpy(np.random.default_rng(ep).permutation(tr)).to(dev)
        tot = []
        for b in perm.split(a.batch):
            s, h, acc, _ = batch_loss(b)
            loss = s + a.hard * h
            opt.zero_grad(set_to_none=True)
            loss.backward()
            opt.step()
            sched.step()
            tot.append([s.item(), h.item(), acc.item()])
        m = np.mean(tot, 0)
        if ep % 5 == 0 or ep == a.epochs:
            print(f"epoch {ep} train soft {m[0]:.4f} hard {m[1]:.4f} top1 {m[2]:.4f}", flush=True)
            evaluate(f"epoch {ep}")
    net.export(a.out, vhash)
    print(f"wrote {a.out}", flush=True)


if __name__ == "__main__":
    main()
