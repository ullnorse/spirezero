"""WP3 done-check (`docs/plans/necro-e2e.md`): token stats of the e2e encoder on 20,000 SMOKE
states (A10 Necrobinder, seeds 23,000,000+), and that the tokens are the same bits in any batch.

States come from random legal play, or with `--checkpoint state.pt` from that learner's policy
(sampled), which reaches deeper floors.

    ~/spirebot/.venv/bin/python python/e2e_token_stats.py [--checkpoint PATH] [--device cuda]
"""

import argparse
import pathlib
import sys

import numpy as np
import torch

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

from sts2 import E2E, Sts2RunEnvs  # noqa: E402
from sts2.e2e import encode as E  # noqa: E402

SMOKE = (23_000_000, 24_000_000)


def states(n, device, checkpoint=None, envs=512, seed=0):
    """`n` (obs, masks) SMOKE rows on `device`."""
    env = Sts2RunEnvs(envs, seed_range=SMOKE, ascension=10, threads=8, **E2E)
    net = None
    if checkpoint:
        from sts2.e2e.net import E2ENet

        s = torch.load(checkpoint, map_location="cpu", weights_only=False)
        net = E2ENet(d=s["config"]["d"], layers=s["config"]["layers"]).to(device)
        net.load_state_dict(s["net"])
        net.eval()
    rng = np.random.default_rng(seed)
    gen = torch.Generator(device=device)
    gen.manual_seed(seed)
    obs, masks, got = [], [], 0
    while got < n:
        o = torch.from_numpy(env.obs.copy()).to(device)
        m = torch.from_numpy(env.masks.view(np.int64).copy()).to(device)
        obs.append(o)
        masks.append(m)
        got += len(o)
        if net is None:
            legal = env.legal()
            a = np.where(legal, rng.random(legal.shape), -1).argmax(1)
        else:
            with torch.no_grad(), torch.autocast(device.type, dtype=torch.bfloat16,
                                                 enabled=device.type == "cuda"):
                logits, _, _ = net.policy(E.encode(o, m))
            u = torch.rand(logits.shape, generator=gen, device=device)
            a = (logits.float() - torch.log((-torch.log(u.clamp(min=1e-30))).clamp(min=1e-30)))
            a = a.argmax(-1).cpu().numpy()
        env.step(a)
    env.close()
    return torch.cat(obs)[:n], torch.cat(masks)[:n]


def bit_stable(obs, masks, sizes=(1, 7, 333, 4096)):
    """Whether every encoder output is identical when the batch is split into `sizes` chunks."""
    full = E.encode(obs, masks)
    per_row = [k for k, v in full.items() if v.dim() >= 1 and v.shape[0] == obs.shape[0]]
    for size in sizes:
        parts = [E.encode(obs[i:i + size], masks[i:i + size]) for i in range(0, len(obs), size)]
        for k in per_row:
            if not torch.equal(torch.cat([p[k] for p in parts]), full[k]):
                return False, f"{k} at batch {size}"
    return True, ""


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--checkpoint")
    ap.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument("--n", type=int, default=20_000)
    args = ap.parse_args()
    dev = torch.device(args.device)
    obs, masks = states(args.n, dev, args.checkpoint)
    tok = E.encode(obs, masks)
    c = tok["count"].float()
    floor = obs[:, 7].float()
    print(f"states {len(obs)} ({'policy ' + args.checkpoint if args.checkpoint else 'random'}),"
          f" mean floor {floor.mean():.2f}, max {int(floor.max())}")
    print(f"tokens mean {c.mean():.2f}  p99 {c.quantile(0.99):.0f}  max {int(c.max())}  "
          f"overflow {int(tok['overflow'].sum())}")
    for phase in range(14):
        m = tok["phase"] == phase
        if m.any():
            print(f"  phase {phase:2d}: {int(m.sum()):6d} states, tokens mean {c[m].mean():.1f}, "
                  f"max {int(c[m].max())}")
    ok, why = bit_stable(obs[:4096], masks[:4096])
    print("bit-stable across batch sizes 1, 7, 333, 4096:", ok, why)


if __name__ == "__main__":
    main()
