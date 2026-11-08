"""Expert iteration on the combat value net (RLB12): fit its win and HP heads to the outcomes
of the search's own play.

The data is `sts2-diff value-collect` (the bot's turn-start observations, each with how its
fight ended under the bot): `obs.bin` rows of the leak-tested observation and `samples.tsv`
(seed, kind, won, HP at the end, max HP). The search scores turn ends with the net, so the
target is P(win) and the HP fraction kept *under the search*, not under the net's greedy
policy (RLB11's PPO targets). Elite samples weigh `--elite`, boss samples `--boss`; the last
`--eval-runs` seeds are held out. The policy head is kept close to the start net (a KL term),
and the start net's own head outputs are mixed into the targets by `--anchor`.

    .venv/bin/python python/rlb12_value_ei.py --data DIR [DIR ...] --init RLB11/last.pt --out DIR
"""

import argparse
import pathlib
import sys

import numpy as np
import torch
import torch.nn.functional as F

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

from sts2 import combat_net  # noqa: E402

W = combat_net.OBS_BYTES + combat_net.STATIC_BYTES


def load(dirs):
    obs, rows = [], []
    for d in dirs:
        o = np.fromfile(pathlib.Path(d) / "obs.bin", np.uint8)
        t = np.loadtxt(pathlib.Path(d) / "samples.tsv", dtype=np.int64, ndmin=2)
        n = min(len(o) // W, len(t))  # a collection still writing may be cut short
        obs.append(o[: n * W].reshape(n, W))
        rows.append(t[:n])
    return np.concatenate(obs), np.concatenate(rows)


@torch.inference_mode()
def heads(net, raw, bs=16384):
    out = [net(raw[i:i + bs]) for i in range(0, len(raw), bs)]
    return tuple(torch.cat([o[k] for o in out]) for k in range(3))


def report(tag, net, raw, win, hp, kind):
    _, wl, hl = heads(net, raw)
    p, q = torch.sigmoid(wl), torch.sigmoid(hl)
    parts = []
    for k, name in enumerate(("common", "elite", "boss")):
        m = kind == k
        if m.any():
            bce = F.binary_cross_entropy(p[m].clamp(1e-6, 1 - 1e-6), win[m]).item()
            parts.append(f"{name} n {int(m.sum())} won {win[m].mean():.3f} pred {p[m].mean():.3f} "
                         f"bce {bce:.4f} hp {hp[m].mean():.3f}/{q[m].mean():.3f}")
    print(f"{tag}: " + " | ".join(parts), flush=True)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--data", nargs="+", required=True)
    p.add_argument("--init", required=True)
    p.add_argument("--out", required=True)
    p.add_argument("--eval-runs", type=int, default=150)
    p.add_argument("--elite", type=float, default=3.0)
    p.add_argument("--boss", type=float, default=6.0)
    p.add_argument("--epochs", type=int, default=4)
    p.add_argument("--lr", type=float, default=3e-5)
    p.add_argument("--minibatch", type=int, default=4096)
    p.add_argument("--anchor", type=float, default=0.0)
    p.add_argument("--kl", type=float, default=1.0)
    p.add_argument("--device", default="cuda")
    args = p.parse_args()
    device = torch.device(args.device)
    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    saved = torch.load(args.init, map_location=device, weights_only=False)
    net = combat_net.Net(saved["hidden"]).to(device)
    net.load_state_dict(saved["model"])
    start = combat_net.Net(saved["hidden"]).to(device)
    start.load_state_dict(saved["model"])
    start.eval()
    obs, rows = load(args.data)
    seeds = rows[:, 0]
    held = np.isin(seeds, np.unique(seeds)[-args.eval_runs:])
    raw = torch.from_numpy(obs).to(device)
    kind = torch.from_numpy(rows[:, 1]).to(device)
    win = torch.from_numpy(rows[:, 2]).float().to(device)
    hp = (torch.from_numpy(rows[:, 3]).float() / torch.from_numpy(rows[:, 4]).clamp(min=1).float()
          ).clamp(0, 1).to(device) * win
    weight = torch.ones_like(win)
    weight[kind == 1] = args.elite
    weight[kind == 2] = args.boss
    tr, ev = torch.from_numpy(~held).to(device), torch.from_numpy(held).to(device)
    pol0, w0, h0 = heads(start, raw)
    tw = (1 - args.anchor) * win + args.anchor * torch.sigmoid(w0)
    th = (1 - args.anchor) * hp + args.anchor * torch.sigmoid(h0)
    idx = torch.nonzero(tr).squeeze(1)
    print(f"{len(raw)} samples, {int(tr.sum())} train, {int(ev.sum())} held out", flush=True)
    report("start eval", net, raw[ev], win[ev], hp[ev], kind[ev])
    opt = torch.optim.Adam(net.parameters(), lr=args.lr, eps=1e-5)
    for epoch in range(1, args.epochs + 1):
        net.train()
        losses = []
        for chunk in idx[torch.randperm(len(idx), device=device)].split(args.minibatch):
            logits, wl, hl = net(raw[chunk])
            wt = weight[chunk]
            vloss = ((F.binary_cross_entropy_with_logits(wl, tw[chunk], reduction="none")
                      + F.binary_cross_entropy_with_logits(hl, th[chunk], reduction="none"))
                     * wt).sum() / wt.sum()
            # Keep the policy head where it was (the search does not read it by default).
            kl = F.kl_div(F.log_softmax(logits, 1), F.log_softmax(pol0[chunk], 1),
                          log_target=True, reduction="batchmean")
            loss = vloss + args.kl * kl
            opt.zero_grad(set_to_none=True)
            loss.backward()
            torch.nn.utils.clip_grad_norm_(net.parameters(), 1.0)
            opt.step()
            losses.append([vloss.item(), kl.item()])
        net.eval()
        m = np.mean(losses, axis=0)
        print(f"epoch {epoch} vloss {m[0]:.4f} kl {m[1]:.4f}", flush=True)
        report(f"epoch {epoch} eval", net, raw[ev], win[ev], hp[ev], kind[ev])
        combat_net.export(net, out / f"e{epoch}.bin", saved["vocab_hash"])
        torch.save({**saved, "model": net.state_dict(), "opt": None,
                    "ei_epoch": epoch, "ei_data": args.data}, out / f"e{epoch}.pt")


if __name__ == "__main__":
    main()
