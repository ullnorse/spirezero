"""Masked PPO for the combat value/policy net on the GPU (RLB11), over PufferLib's native env.

The env is `sts2.env.Sts2PufferEnv` (a `pufferlib.PufferEnv`) with the static part appended to
each observation, over real fight starts the bot met on its train-seed runs (`sts2-diff
collect-starts`, sharded and weighted by `python/rlb11_starts.py`), each episode with its hidden
part resampled (`blindfold`). The advantages are PufferLib's kernel (`compute_puff_advantage`,
importance 1: GAE). The net (`sts2/combat_net.py`) has a win head and an HP head (the HP
fraction kept, 0 for a loss), each trained on its own lambda-return in probability space; the
critic and the policy's reward are `win + HP_BONUS * hp`.

    .venv/bin/python python/train_combat_puffer.py --shards DIR --eval DIR --out DIR
"""

import argparse
import glob
import json
import pathlib
import sys
import time

import numpy as np
import torch
import torch.nn.functional as F

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import pufferlib  # noqa: E402,F401
import pufferlib._C  # noqa: E402,F401  (registers torch.ops.pufferlib)
from sts2 import combat_net  # noqa: E402
from sts2.combat_net import HP_BONUS, Net, unpack_masks  # noqa: E402
from sts2.env import Sts2PufferEnv  # noqa: E402


def make_env(path, n, seed, threads):
    env = Sts2PufferEnv(n, starts=path, seed=seed, threads=threads, with_static=True)
    env.batch.blindfold(seed * 7919 + 1)
    env.reset()
    return env


def masks_of(env, device):
    return unpack_masks(torch.from_numpy(env.batch.masks.view(np.int64)).to(device))


def advantage(values, rewards, dones, gamma, lam):
    """PufferLib's advantage over (envs × horizon) rows; the last column has none."""
    adv = torch.zeros_like(values)
    torch.ops.pufferlib.compute_puff_advantage(
        values.contiguous(), rewards.contiguous(), dones.contiguous(),
        torch.ones_like(values), adv, gamma, lam, 1.0, 1.0)
    return adv


@torch.inference_mode()
def evaluate(net, path, device, episodes, n=512, threads=4, seed=99):
    """Greedy play of `episodes` episodes from `path`: wins, mean predicted P(win) at the
    first observation of each, and the mean HP fraction of the wins."""
    env = make_env(path, n, seed, threads)
    max_hp = np.maximum(env.observations[:, 2:4].copy().view(np.int16)[:, 0], 1)
    pred_start = torch.sigmoid(net(torch.from_numpy(env.observations).to(device))[1]).cpu()
    start_pred = pred_start.numpy().copy()
    done = wins = 0
    preds, hps = [], []
    while done < episodes:
        raw = torch.from_numpy(env.observations).to(device)
        logits, _, _ = net(raw)
        logits = logits.masked_fill(~masks_of(env, device), -1e9)
        action = logits.argmax(1).cpu().numpy()
        prev_max = max_hp.copy()
        env.step(action)
        d = env.batch.done.astype(bool)
        if d.any():
            won = env.batch.won[d] == 1
            done += int(d.sum())
            wins += int(won.sum())
            preds.extend(start_pred[d].tolist())
            hps.extend((env.batch.final_hp[d][won] / prev_max[d][won]).tolist())
            new = torch.sigmoid(net(torch.from_numpy(env.observations).to(device))[1]).cpu()
            start_pred[d] = new.numpy()[d]
            max_hp[d] = np.maximum(env.observations[d, 2:4].copy().view(np.int16)[:, 0], 1)
    env.close()
    return {"episodes": done, "win": wins / max(done, 1),
            "pred": float(np.mean(preds)), "hp": float(np.mean(hps)) if hps else 0.0}


def train(args):
    torch.manual_seed(args.seed)
    device = torch.device(args.device)
    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    shards = sorted(glob.glob(f"{args.shards}/shard-*.jsonl"))
    net = Net(args.hidden).to(device)
    opt = torch.optim.Adam(net.parameters(), lr=args.lr, eps=1e-5)
    start_update, steps_done = 0, 0
    if args.resume:
        saved = torch.load(args.resume, map_location=device, weights_only=False)
        net.load_state_dict(saved["model"])
        if "opt" in saved and not args.fresh_opt:
            opt.load_state_dict(saved["opt"])
        start_update, steps_done = saved.get("update", 0), saved.get("steps", 0)
    n, T = args.envs, args.horizon
    shard_i = start_update // args.shard_every
    env = make_env(shards[shard_i % len(shards)], n, args.seed + shard_i, args.threads)
    vocab_hash = int(env.batch.lib.sts2_vocab_hash())
    obs_w = env.observations.shape[1]
    raw_buf = torch.zeros((n, T, obs_w), dtype=torch.uint8, device=device)
    mask_buf = torch.zeros((n, T, combat_net.ACTIONS), dtype=torch.bool, device=device)
    act_buf = torch.zeros((n, T), dtype=torch.long, device=device)
    logp_buf = torch.zeros((n, T), device=device)
    vw_buf = torch.zeros((n, T), device=device)
    vh_buf = torch.zeros((n, T), device=device)
    rw_buf = np.zeros((n, T), np.float32)
    rh_buf = np.zeros((n, T), np.float32)
    d_buf = np.zeros((n, T), np.float32)
    rw = np.zeros(n, np.float32)
    rh = np.zeros(n, np.float32)
    dn = np.zeros(n, np.float32)
    log = open(out / "curve.tsv", "a")
    started = time.perf_counter()
    total_updates = args.total_steps // (n * (T - 1))
    episodes = wins = 0
    for update in range(start_update + 1, total_updates + 1):
        if (update - 1) // args.shard_every != shard_i:
            env.close()
            shard_i = (update - 1) // args.shard_every
            env = make_env(shards[shard_i % len(shards)], n, args.seed + shard_i, args.threads)
            rw[:] = rh[:] = dn[:] = 0
        frac = 1.0 - (update - 1) / total_updates
        for g in opt.param_groups:
            g["lr"] = args.lr * max(frac, 0.05)
        ent_coef = args.entropy_end + (args.entropy - args.entropy_end) * frac
        t0 = time.perf_counter()
        env_time = 0.0
        with torch.inference_mode():
            for t in range(T):
                raw = torch.from_numpy(env.observations).to(device, non_blocking=True)
                mask = masks_of(env, device)
                logits, wl, hl = net(raw)
                logits = logits.masked_fill(~mask, -1e9)
                dist = torch.distributions.Categorical(logits=logits)
                action = dist.sample()
                raw_buf[:, t] = raw
                mask_buf[:, t] = mask
                act_buf[:, t] = action
                logp_buf[:, t] = dist.log_prob(action)
                vw_buf[:, t] = torch.sigmoid(wl)
                vh_buf[:, t] = torch.sigmoid(hl)
                rw_buf[:, t] = rw
                rh_buf[:, t] = rh
                d_buf[:, t] = dn
                max_hp = np.maximum(env.observations[:, 2:4].copy().view(np.int16)[:, 0], 1)
                te = time.perf_counter()
                env.step(action.cpu().numpy())
                env_time += time.perf_counter() - te
                d = env.batch.done.astype(np.float32)
                won = (env.batch.won == 1).astype(np.float32) * d
                rw = won
                rh = won * np.clip(env.batch.final_hp / max_hp, 0, 1).astype(np.float32)
                dn = d
                episodes += int(d.sum())
                wins += int(won.sum())
        t_roll = time.perf_counter() - t0
        vw, vh = vw_buf.cpu(), vh_buf.cpu()
        d_t = torch.from_numpy(d_buf)
        adv_w = advantage(vw, torch.from_numpy(rw_buf), d_t, 1.0, args.lam)
        adv_h = advantage(vh, torch.from_numpy(rh_buf), d_t, 1.0, args.lam)
        ret_w = (adv_w + vw).clamp(0, 1).to(device)
        ret_h = (adv_h + vh).clamp(0, 1).to(device)
        adv = (adv_w + HP_BONUS * adv_h).to(device)
        # The last column has no advantage: train on the first T - 1.
        flat = lambda x: x[:, : T - 1].reshape(-1, *x.shape[2:])  # noqa: E731
        b_raw, b_mask, b_act, b_logp = flat(raw_buf), flat(mask_buf), flat(act_buf), flat(logp_buf)
        b_adv, b_rw, b_rh = flat(adv), flat(ret_w), flat(ret_h)
        b_adv = (b_adv - b_adv.mean()) / (b_adv.std() + 1e-8)
        count = b_raw.shape[0]
        stats = []
        for _ in range(args.epochs):
            order = torch.randperm(count, device=device)
            for chunk in order.split(args.minibatch):
                logits, wl, hl = net(b_raw[chunk])
                dist = torch.distributions.Categorical(
                    logits=logits.masked_fill(~b_mask[chunk], -1e9))
                logp = dist.log_prob(b_act[chunk])
                ratio = torch.exp(logp - b_logp[chunk])
                a = b_adv[chunk]
                pg = -torch.minimum(ratio * a, ratio.clamp(1 - args.clip, 1 + args.clip) * a).mean()
                vloss = (F.binary_cross_entropy_with_logits(wl, b_rw[chunk])
                         + F.binary_cross_entropy_with_logits(hl, b_rh[chunk]))
                ent = (dist.entropy() * (b_mask[chunk].sum(1) > 1)).mean()
                loss = pg + args.value_coef * vloss - ent_coef * ent
                opt.zero_grad(set_to_none=True)
                loss.backward()
                torch.nn.utils.clip_grad_norm_(net.parameters(), args.max_grad)
                opt.step()
                stats.append([float(pg), float(vloss), float(ent),
                              float(((ratio - 1).abs() > args.clip).float().mean())])
        steps_done += n * (T - 1)
        s = np.mean(stats, axis=0)
        sps = n * T / (time.perf_counter() - t0)
        line = (f"{update}\t{steps_done}\t{time.perf_counter() - started:.0f}\t{sps:.0f}"
                f"\t{n * T / max(env_time, 1e-9):.0f}\t{n * T / t_roll:.0f}\t{wins / max(episodes, 1):.4f}\t{episodes}"
                f"\t{s[0]:+.4f}\t{s[1]:.4f}\t{s[2]:.3f}\t{s[3]:.3f}")
        print(line, flush=True)
        episodes = wins = 0
        if update % args.eval_every == 0 or update == total_updates:
            row = {"update": update, "steps": steps_done}
            for kind in ("common", "elite", "boss"):
                path = f"{args.eval}/{kind}.jsonl"
                if pathlib.Path(path).exists():
                    row[kind] = evaluate(net, path, device, args.eval_episodes)
            print("eval", json.dumps(row), flush=True)
            log.write("eval\t" + json.dumps(row) + "\n")
            ckpt = {"model": net.state_dict(), "opt": opt.state_dict(), "update": update,
                    "steps": steps_done, "vocab_hash": vocab_hash, "hidden": args.hidden,
                    "eval": row}
            torch.save(ckpt, out / f"u{update:05d}.pt")
            torch.save(ckpt, out / "last.pt")
            combat_net.export(net, out / f"u{update:05d}.bin", vocab_hash)
        log.write(line + "\n")
        log.flush()
    env.close()


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--shards", required=True)
    p.add_argument("--eval", required=True)
    p.add_argument("--out", required=True)
    p.add_argument("--resume")
    p.add_argument("--fresh-opt", action="store_true")
    p.add_argument("--envs", type=int, default=4096)
    p.add_argument("--threads", type=int, default=12)
    p.add_argument("--horizon", type=int, default=65)
    p.add_argument("--total-steps", type=int, default=300_000_000)
    p.add_argument("--shard-every", type=int, default=40)
    p.add_argument("--hidden", type=int, default=combat_net.HIDDEN)
    p.add_argument("--lr", type=float, default=3e-4)
    p.add_argument("--lam", type=float, default=0.95)
    p.add_argument("--clip", type=float, default=0.2)
    p.add_argument("--epochs", type=int, default=2)
    p.add_argument("--minibatch", type=int, default=16384)
    p.add_argument("--entropy", type=float, default=0.01)
    p.add_argument("--entropy-end", type=float, default=0.002)
    p.add_argument("--value-coef", type=float, default=1.0)
    p.add_argument("--max-grad", type=float, default=1.0)
    p.add_argument("--eval-every", type=int, default=50)
    p.add_argument("--eval-episodes", type=int, default=2048)
    p.add_argument("--seed", type=int, default=0)
    p.add_argument("--device", default="cuda")
    train(p.parse_args())


if __name__ == "__main__":
    main()
