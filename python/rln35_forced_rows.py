"""RLN35 part B: forced rows (one legal action) and near-deterministic rows in the e2e batch.

Resumes a checkpoint's env position with its weights (no update), collects `--iters` batches and
tallies per row type (combat, run): rows, rows with exactly one legal action, rows whose policy
puts >= 0.99 on one action, and each group's share of tokens (`width`, the update's cost proxy).
Run it from a scratch copy of the checkpoint: it only reads `--state`.

    STS2_FFI_LIB=... STS2_FFI_BUILD_ID=... ~/spirebot/.venv/bin/python python/rln35_forced_rows.py \\
        --state target/rln35/state.pt --iters 4 --out target/rln35/forced.json
"""

import argparse
import json
import pathlib
import shutil
import sys
import tempfile

import numpy as np
import torch

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import train_e2e as T  # noqa: E402


def popcount(masks):
    x = masks.view(torch.int64) if masks.dtype != torch.int64 else masks
    n = torch.zeros(x.shape[:-1], dtype=torch.int64, device=x.device)
    for w in range(x.shape[-1]):
        v = x[..., w]
        for b in range(64):
            n += (v >> b) & 1
    return n


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--state", required=True)
    ap.add_argument("--iters", type=int, default=4)
    ap.add_argument("--steps", type=int, default=16)
    ap.add_argument("--threads", type=int, default=2)
    ap.add_argument("--device", default="cuda")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()
    tmp = pathlib.Path(tempfile.mkdtemp(dir=pathlib.Path(args.out).parent))
    shutil.copy(args.state, tmp / "state.pt")
    cfg = T.Config.from_dict(torch.load(tmp / "state.pt", weights_only=False)["config"])
    cfg.threads, cfg.device, cfg.compile, cfg.overlap = args.threads, args.device, False, False
    cfg.micro, cfg.steps = 256, args.steps
    if args.device == "cuda":  # share the GPU with live training: stay under 3 GB
        torch.cuda.set_per_process_memory_fraction(3.0 / 15.5)
    learner = T.Learner(cfg, tmp, resume=True)
    rec = []
    orig = learner._forward

    def wrapped(*a, **k):
        tok, ptok, logits, v = orig(*a, **k)
        lg = logits.detach().float()
        pmax = torch.softmax(lg, -1).max(-1).values
        rec.append((popcount(a[1]).cpu(), pmax.cpu(), tok["combat"].cpu(), tok["width"].cpu()))
        return tok, ptok, logits, v

    learner._forward = wrapped
    acc = {t: np.zeros(7) for t in ("combat", "run")}  # rows, forced, p99, forced|p99, tok, tok forced, tok p99
    for it in range(args.iters):
        rec.clear()
        learner._practice_apply()
        buf = learner._alloc()
        learner._collect_into(buf, learner.net, learner.gen)
        del buf
        torch.cuda.empty_cache()
        rec_rows = rec[:cfg.steps]  # the per-step forwards (bootstrap forwards follow)
        legal = torch.stack([r[0] for r in rec_rows]).flatten()
        pmax = torch.stack([r[1] for r in rec_rows]).flatten()
        combat = torch.stack([r[2] for r in rec_rows]).flatten().bool()
        width = torch.stack([r[3] for r in rec_rows]).flatten().double()
        live = legal > 0  # an empty mask is no decision and leaves the update
        forced = live & (legal == 1)
        p99 = live & (pmax >= 0.99)
        for name, sel in (("combat", combat), ("run", ~combat)):
            s = sel & live
            row = [s.sum(), (s & forced).sum(), (s & p99).sum(), (s & (forced | p99)).sum(),
                   width[s].sum(), width[s & forced].sum(), width[s & p99].sum()]
            acc[name] += np.array([float(x) for x in row])
        print(f"collect {it + 1}/{args.iters} rows {int(legal.numel())}", flush=True)
    out = {}
    for name, a in acc.items():
        out[name] = dict(rows=int(a[0]), forced=int(a[1]), p99=int(a[2]), forced_or_p99=int(a[3]),
                         tokens=a[4], tok_forced=a[5], tok_p99=a[6])
    tot = sum(acc.values())
    out["all"] = dict(rows=int(tot[0]), forced=int(tot[1]), p99=int(tot[2]),
                      forced_or_p99=int(tot[3]), tokens=tot[4], tok_forced=tot[5], tok_p99=tot[6])
    for v in out.values():
        v["forced_row_share"] = v["forced"] / v["rows"]
        v["forced_token_share"] = v["tok_forced"] / v["tokens"]
        v["p99_row_share"] = v["p99"] / v["rows"]
        v["p99_token_share"] = v["tok_p99"] / v["tokens"]
        v["mean_width"] = v["tokens"] / v["rows"]
    pathlib.Path(args.out).write_text(json.dumps(out, indent=1))
    print(json.dumps(out, indent=1))
    learner.close()
    shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    main()
