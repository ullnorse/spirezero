"""Expert iteration for the AlphaZero tree (RLB13): fit the combat net's policy head to the
tree's root visits and its win and HP heads to the fights' outcomes under the tree.

The data is `sts2-diff az-collect`: `obs.bin` (the value net's input rows), `visits.bin` (f32
visits by action index, -1 where the root had no edge) and `samples.tsv` (seed, kind, won, HP
at the end, max HP). The policy target is the visit distribution over the root's edges, with
the logits masked to them; the last `--eval-runs` seeds are held out.

    .venv/bin/python python/rlb13_az_ei.py --data DIR [DIR ...] --init RLB11/run2/last.pt --out DIR
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
ACTIONS = 140  # sts2_rl::ACTIONS: the visits' width


def load(dirs):
    obs, vis, rows = [], [], []
    for d in dirs:
        d = pathlib.Path(d)
        t = np.loadtxt(d / "samples.tsv", dtype=np.int64, ndmin=2)
        o = np.fromfile(d / "obs.bin", np.uint8)
        v = np.fromfile(d / "visits.bin", np.float32)
        a = ACTIONS
        n = min(len(o) // W, len(t), len(v) // a)
        obs.append(o[: n * W].reshape(n, W))
        vis.append(v[: n * a].reshape(n, a))
        rows.append(t[:n])
    return np.concatenate(obs), np.concatenate(vis), np.concatenate(rows)


def losses(net, raw, vis, win, hp):
    logits, wl, hl = net(raw)
    legal = vis >= 0
    target = vis.clamp(min=0)
    target = target / target.sum(1, keepdim=True).clamp(min=1)
    logp = F.log_softmax(logits.masked_fill(~legal, -1e9), 1)
    pl = -(target * logp.masked_fill(~legal, 0)).sum(1)
    vl = (F.binary_cross_entropy_with_logits(wl, win, reduction="none")
          + F.binary_cross_entropy_with_logits(hl, hp, reduction="none"))
    top = (logp.argmax(1) == target.argmax(1)).float()
    return pl, vl, top


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--data", nargs="+", required=True)
    p.add_argument("--init", required=True)
    p.add_argument("--out", required=True)
    p.add_argument("--eval-runs", type=int, default=60)
    p.add_argument("--epochs", type=int, default=6)
    p.add_argument("--lr", type=float, default=3e-5)
    p.add_argument("--minibatch", type=int, default=2048)
    p.add_argument("--value", type=float, default=1.0, help="weight of the value losses")
    p.add_argument("--anchor", action="store_true",
                   help="value targets are the start net's own outputs (only the prior learns)")
    p.add_argument("--device", default="cuda")
    args = p.parse_args()
    device = torch.device(args.device)
    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    saved = torch.load(args.init, map_location=device, weights_only=False)
    net = combat_net.Net(saved["hidden"]).to(device)
    net.load_state_dict(saved["model"])
    obs, vis, rows = load(args.data)
    seeds = rows[:, 0]
    held = np.isin(seeds, np.unique(seeds)[-args.eval_runs:])
    raw = torch.from_numpy(obs).to(device)
    vis = torch.from_numpy(vis).to(device)
    win = torch.from_numpy(rows[:, 2]).float().to(device)
    hp = (torch.from_numpy(rows[:, 3]).float() / torch.from_numpy(rows[:, 4]).clamp(min=1).float()
          ).clamp(0, 1).to(device) * win
    if args.anchor:
        with torch.inference_mode():
            outs = [net(raw[i:i + 8192]) for i in range(0, len(raw), 8192)]
        tw = torch.sigmoid(torch.cat([o[1] for o in outs])).clone()
        th = torch.sigmoid(torch.cat([o[2] for o in outs])).clone()
    else:
        tw, th = win, hp
    tr = torch.nonzero(torch.from_numpy(~held)).squeeze(1).to(device)
    ev = torch.nonzero(torch.from_numpy(held)).squeeze(1).to(device)
    print(f"{len(raw)} samples, {len(tr)} train, {len(ev)} held out", flush=True)

    @torch.inference_mode()
    def report(tag):
        net.eval()
        pl, vl, top = losses(net, raw[ev], vis[ev], win[ev], hp[ev])
        print(f"{tag}: held-out policy CE {pl.mean():.4f} top-1 {top.mean():.3f} "
              f"value {vl.mean():.4f}", flush=True)

    report("start")
    opt = torch.optim.Adam(net.parameters(), lr=args.lr, eps=1e-5)
    for epoch in range(1, args.epochs + 1):
        net.train()
        for chunk in tr[torch.randperm(len(tr), device=device)].split(args.minibatch):
            pl, vl, _ = losses(net, raw[chunk], vis[chunk], tw[chunk], th[chunk])
            loss = pl.mean() + args.value * vl.mean()
            opt.zero_grad(set_to_none=True)
            loss.backward()
            torch.nn.utils.clip_grad_norm_(net.parameters(), 1.0)
            opt.step()
        report(f"epoch {epoch}")
        combat_net.export(net, out / f"e{epoch}.bin", saved["vocab_hash"])
        torch.save({**saved, "model": net.state_dict(), "opt": None, "ei_epoch": epoch,
                    "ei_data": args.data}, out / f"e{epoch}.pt")


if __name__ == "__main__":
    main()
