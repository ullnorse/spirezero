"""RLN31: full-view rows under a pinned policy, and the encoder's token and cost profile on them.

`collect`: plays `--envs` runs (A10 Necrobinder, seeds 300M+, no practice) with a checkpoint's
policy (sampled, its own inputs), with `obs_full` rows, and keeps every `--every`-th step's rows
(obs, masks, priv) in an `.npz`.

`profile`: on those rows, per token kind the mean and max count with the full view, the widest
row, the overflow rate, and the time of `encode` (off / on) and of a forward+backward at
`--micro` rows (sorted micros as the learner cuts them).

    ~/spirebot/.venv/bin/python python/rln31_rows.py collect --checkpoint PATH --out rows.npz
    ~/spirebot/.venv/bin/python python/rln31_rows.py profile --rows rows.npz
"""

import argparse
import pathlib
import sys
import time

import numpy as np
import torch

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

from sts2 import E2E, Sts2RunEnvs  # noqa: E402
from sts2.e2e import encode as E  # noqa: E402

SEEDS = (300_000_000, 400_000_000)


def collect(args):
    from train_e2e import net_of

    dev = torch.device(args.device)
    s = torch.load(args.checkpoint, map_location="cpu", weights_only=False)
    cfg = s["config"]
    net = net_of(cfg).to(dev)
    net.load_state_dict(s["net"])
    net.eval()
    full_net = bool(cfg.get("obs_full", False))
    env = Sts2RunEnvs(args.envs, seed_range=SEEDS, ascension=10, character="Necrobinder",
                      combat="agent", threads=args.threads, priv=True, obs_v2=True,
                      obs_full=True, **E2E)
    E.check_layout(env.layout, v2=True, full=True)
    gen = torch.Generator(device=dev)
    gen.manual_seed(0)
    keep = dict(obs=[], masks=[], priv=[], floor=[])
    t0 = time.perf_counter()
    for t in range(args.steps):
        o = torch.from_numpy(env.obs[:, :E.OBS_BYTES_FULL].copy()).to(dev)
        m = torch.from_numpy(env.masks.view(np.int64).copy()).to(dev)
        if t % args.every == args.every - 1:
            keep["obs"].append(env.obs[:, :E.OBS_BYTES_FULL].copy())
            keep["masks"].append(env.masks.view(np.int64).copy())
            keep["priv"].append(env.priv.copy())
            keep["floor"].append(env.obs[:, 7].copy())
        with torch.no_grad(), torch.autocast(dev.type, dtype=torch.bfloat16,
                                             enabled=dev.type == "cuda"):
            logits, _, _ = net.policy(E.encode(o, m, v2=True, full=full_net))
        u = torch.rand(logits.shape, generator=gen, device=dev)
        a = (logits.float() - torch.log((-torch.log(u.clamp(min=1e-30))).clamp(min=1e-30)))
        env.step(a.argmax(-1).cpu().numpy())
        if t % 200 == 0:
            print(f"step {t} {time.perf_counter() - t0:.0f}s max floor {env.obs[:, 7].max()}",
                  flush=True)
    env.close()
    np.savez(args.out, **{k: np.concatenate(v) for k, v in keep.items()})
    print(f"saved {sum(len(x) for x in keep['obs'])} rows -> {args.out}")


def _timed(fn, reps):
    torch.cuda.synchronize()
    t0 = time.perf_counter()
    for _ in range(reps):
        fn()
    torch.cuda.synchronize()
    return (time.perf_counter() - t0) / reps


def profile(args):
    from train_e2e import net_of
    from sts2.e2e.net import E2ENet

    dev = torch.device(args.device)
    z = np.load(args.rows)
    obs = torch.from_numpy(z["obs"]).to(dev)
    masks = torch.from_numpy(z["masks"]).to(dev)
    priv = torch.from_numpy(z["priv"]).to(dev)
    n = obs.shape[0]
    print(f"{n} rows, floors up to {int(z['obs'][:, 7].max())}")
    names = E.KINDS + E.FULL_KINDS
    counts = {False: [], True: []}
    widths = {False: [], True: []}
    over = {False: 0, True: 0}
    for full in (False, True):
        for i in range(0, n, 8192):
            tok = E.encode(obs[i:i + 8192], masks[i:i + 8192], v2=True, full=full)
            k = torch.where(tok["valid"], tok["kind"], -1)
            counts[full].append(torch.stack([(k == j).sum(1) for j in range(len(names))], 1).cpu())
            widths[full].append(tok["width"].cpu())
            over[full] += int(tok["overflow"].sum())
        counts[full] = torch.cat(counts[full]).float()
        widths[full] = torch.cat(widths[full])
    combat = obs[:, 3].cpu() == E.PHASE_COMBAT
    print(f"{'kind':14s} {'off mean':>9s} {'on mean':>9s} {'on max':>7s}  (all rows)")
    for j, name in enumerate(names):
        a, b = counts[False][:, j], counts[True][:, j]
        if b.max() > 0:
            print(f"{name:14s} {a.mean():9.2f} {b.mean():9.2f} {int(b.max()):7d}")
    for full in (False, True):
        w = widths[full].float()
        print(f"full={full}: width mean {w.mean():.1f} (combat {w[combat].mean():.1f}) "
              f"p99 {w.quantile(0.99):.0f} max {int(w.max())}; overflow {over[full]} "
              f"({over[full] / n:.2e})")
    if args.counts_only:
        return

    # Encode time per 2,048-row batch (the collect's shape).
    for full in (False, True):
        o, m = obs[:2048], masks[:2048]
        E.encode(o, m, v2=True, full=full)
        dt = _timed(lambda: E.encode(o, m, v2=True, full=full), 10)
        print(f"encode 2048 rows full={full}: {dt * 1e3:.1f} ms")
    # Net forward (collect: 2,048 rows, no sort) and forward+backward (update: sorted micros).
    for full in (False, True):
        cfg = dict(d=256, layers=4, obs_v2=True, card_stats=True, emb_std=0.4, obs_full=full)
        net = net_of(cfg).to(dev)
        o, m, p = obs[:2048], masks[:2048], priv[:2048]
        tok = E.encode(o, m, v2=True, full=full)
        tok["y_rows"] = E2ENet.y_rows_of((o[:, 3] == E.PHASE_MAP).cpu().numpy(), dev)
        ptok = E.encode_priv(p)

        def fwd():
            with torch.no_grad(), torch.autocast("cuda", dtype=torch.bfloat16):
                net(tok, ptok)
        fwd()
        dt = _timed(fwd, 10)
        print(f"collect forward 2048 rows full={full}: {dt * 1e3:.1f} ms "
              f"(t {int(tok['width'].max())})")
        # Update: 8,192-row minibatches sorted by width, micros of `micro`.
        rows = min(n, 65536)
        w = widths[full][:rows].numpy()
        perm = np.random.default_rng(0).permutation(rows)
        torch.cuda.reset_peak_memory_stats()
        t_sum, steps = 0, 0
        torch.cuda.synchronize()
        t0 = time.perf_counter()
        for mb in range(0, rows, 8192):
            part = perm[mb:mb + 8192]
            part = part[np.argsort(w[part], kind="stable")]
            for mc in range(0, len(part), args.micro):
                idx_h = part[mc:mc + args.micro]
                idx = torch.from_numpy(idx_h).to(dev)
                tk = E.encode(obs[idx], masks[idx], v2=True, full=full)
                tk["t"] = int(w[idx_h].max())
                tk["y_rows"] = E2ENet.y_rows_of((obs[idx, 3] == E.PHASE_MAP).cpu().numpy(), dev)
                with torch.autocast("cuda", dtype=torch.bfloat16):
                    logits, v = net(tk, E.encode_priv(priv[idx]))
                (logits.float().nan_to_num(neginf=0).sum() * 0 + v.float().sum()).backward()
                t_sum += tk["t"]
                steps += 1
        torch.cuda.synchronize()
        dt = time.perf_counter() - t0
        print(f"update fwd+bwd (incl. encode) {rows} rows full={full}: {rows / dt:.0f} rows/s, "
              f"mean micro t {t_sum / steps:.1f}, peak {torch.cuda.max_memory_allocated() / 1e9:.2f} GB")
        net.zero_grad(set_to_none=True)
        del net


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("collect")
    c.add_argument("--checkpoint", required=True)
    c.add_argument("--out", required=True)
    c.add_argument("--envs", type=int, default=2048)
    c.add_argument("--steps", type=int, default=1536)
    c.add_argument("--every", type=int, default=16)
    c.add_argument("--threads", type=int, default=14)
    c.add_argument("--device", default="cuda")
    p = sub.add_parser("profile")
    p.add_argument("--rows", required=True)
    p.add_argument("--micro", type=int, default=1024)
    p.add_argument("--device", default="cuda")
    p.add_argument("--counts-only", action="store_true")
    args = ap.parse_args()
    (collect if args.cmd == "collect" else profile)(args)


if __name__ == "__main__":
    main()
