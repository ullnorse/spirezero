"""Masked PPO on the combat env (`docs/specs/a10-training.md`; task RLB4).

The combat env is the short-horizon learner: an episode is one fight, the observation is the
engine's 972-byte combat observation plus its 274-byte static part, the action is one of 140
indices, and the mask is the engine's legal list. The reward is +1 for a won fight and -1 for a
loss (the env's own terminal reward); evaluation is the unshaped win rate over a fixed start
batch.

    cargo build --profile ffi -p sts2-ffi
    python3 -m venv .venv && .venv/bin/pip install numpy torch
    .venv/bin/python python/train_combat_ppo.py --updates 200

Nothing here decides a game rule: the env steps the engine and the policy only reads its
observation and mask.
"""

import argparse
import pathlib
import sys
import time

import numpy as np
import torch
import torch.nn as nn
from torch.distributions import Categorical

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

from sts2 import ffi  # noqa: E402
from sts2.env import Sts2Envs  # noqa: E402

OBS_BYTES = 0  # set from the env at start


def encode(env):
    """The observation bytes and the static part as one float tensor, scaled to [0, 1]."""
    raw = np.concatenate((env.obs, env.static_obs), axis=1).astype(np.float32)
    return torch.from_numpy(raw / 255.0)


def legal_mask(env):
    """The env's masks as an `n × actions` bool array (a copy)."""
    bits = np.unpackbits(env.masks.view(np.uint8), axis=1, bitorder="little")
    return bits[:, : env.actions].astype(bool)


class Net(nn.Module):
    def __init__(self, obs_dim, actions, hidden=256):
        super().__init__()
        self.body = nn.Sequential(
            nn.Linear(obs_dim, hidden),
            nn.ReLU(),
            nn.Linear(hidden, hidden),
            nn.ReLU(),
        )
        self.policy = nn.Linear(hidden, actions)
        self.value = nn.Linear(hidden, 1)

    def forward(self, obs):
        h = self.body(obs)
        return self.policy(h), self.value(h).squeeze(-1)


def masked_logits(logits, mask):
    return logits.masked_fill(~mask, -1.0e9)


def select(net, obs, mask, deterministic=False):
    """One action per env and its log-probability, under the mask."""
    logits, value = net(obs)
    logits = masked_logits(logits, mask)
    dist = Categorical(logits=logits)
    action = torch.argmax(logits, dim=-1) if deterministic else dist.sample()
    return action, dist.log_prob(action), value


def evaluate(net, device, episodes=512, n=64, seed=10_000, random_baseline=False):
    env = Sts2Envs(n, seed=seed)
    obs = encode(env).to(device)
    rng = np.random.default_rng(seed)
    done_count = 0
    wins = 0
    steps = 0
    started = time.perf_counter()
    try:
        while done_count < episodes:
            if random_baseline:
                legal = legal_mask(env)
                actions = np.argmax(
                    np.where(legal, rng.random(legal.shape), -1.0), axis=1
                ).astype(np.uint16)
            else:
                with torch.inference_mode():
                    actions = select(
                        net, obs, torch.from_numpy(legal_mask(env)).to(device), deterministic=True
                    )[0]
                actions = actions.cpu().numpy().astype(np.uint16)
            env.step(actions)
            steps += 1
            done = env.done.astype(bool)
            done_count += int(done.sum())
            wins += int(env.won[done].sum())
            obs = encode(env).to(device)
    finally:
        env.close()
    return {
        "episodes": done_count,
        "wins": wins,
        "win_rate": wins / max(done_count, 1),
        "steps": steps,
        "steps_per_s": steps * n / max(time.perf_counter() - started, 1e-9),
    }


def train(args):
    torch.manual_seed(args.seed)
    np.random.seed(args.seed)
    device = torch.device(args.device)
    env = Sts2Envs(args.envs, seed=args.seed)
    obs_dim = int(env.obs.shape[1] + env.static_obs.shape[1])
    actions = int(env.actions)
    net = Net(obs_dim, actions, args.hidden).to(device)
    opt = torch.optim.Adam(net.parameters(), lr=args.lr, eps=1e-5)
    obs = encode(env).to(device)
    mask = torch.from_numpy(legal_mask(env)).to(device)
    best = -1.0
    started = time.perf_counter()

    for update in range(1, args.updates + 1):
        batch = {
            "obs": [], "masks": [], "actions": [], "logp": [],
            "values": [], "rewards": [], "dones": [],
        }
        episodes = wins = 0
        for _ in range(args.steps):
            with torch.inference_mode():
                action, logp, value = select(net, obs, mask)
            batch["obs"].append(obs)
            batch["masks"].append(mask)
            batch["actions"].append(action)
            batch["logp"].append(logp)
            batch["values"].append(value)
            env.step(action.cpu().numpy().astype(np.uint16))
            done = env.done.astype(bool)
            ended = int(done.sum())
            episodes += ended
            wins += int(env.won[done].sum())
            reward = torch.zeros(args.envs, device=device)
            if ended:
                won = torch.from_numpy(env.won.astype(np.float32)).to(device)
                reward = torch.where(
                    torch.from_numpy(done).to(device), won * 2.0 - 1.0, reward
                )
            batch["rewards"].append(reward)
            batch["dones"].append(torch.from_numpy(done.astype(np.float32)).to(device))
            obs = encode(env).to(device)
            mask = torch.from_numpy(legal_mask(env)).to(device)

        with torch.inference_mode():
            _, next_value = net(obs)
        values = torch.stack(batch["values"]).detach()
        rewards = torch.stack(batch["rewards"]).detach()
        dones = torch.stack(batch["dones"]).detach()
        advantages = torch.zeros_like(rewards)
        last = torch.zeros(args.envs, device=device)
        for t in reversed(range(args.steps)):
            next_v = next_value if t == args.steps - 1 else values[t + 1]
            delta = rewards[t] + args.gamma * next_v * (1.0 - dones[t]) - values[t]
            last = delta + args.gamma * args.lam * (1.0 - dones[t]) * last
            advantages[t] = last
        returns = advantages + values
        adv = advantages.flatten()
        adv = (adv - adv.mean()) / (adv.std() + 1e-8)
        b_obs = torch.stack(batch["obs"]).reshape(-1, obs_dim)
        b_masks = torch.stack(batch["masks"]).reshape(-1, actions)
        b_actions = torch.stack(batch["actions"]).reshape(-1)
        b_logp = torch.stack(batch["logp"]).reshape(-1).detach()
        b_returns = returns.reshape(-1)
        b_values = values.reshape(-1)

        count = b_obs.shape[0]
        losses = []
        for _ in range(args.epochs):
            order = torch.randperm(count, device=device)
            for chunk in order.chunk(args.minibatches):
                logits, value = net(b_obs[chunk])
                dist = Categorical(logits=masked_logits(logits, b_masks[chunk]))
                logp = dist.log_prob(b_actions[chunk])
                ratio = torch.exp(logp - b_logp[chunk])
                surr = torch.minimum(
                    ratio * adv[chunk],
                    torch.clamp(ratio, 1.0 - args.clip, 1.0 + args.clip) * adv[chunk],
                )
                value_loss = (value - b_returns[chunk]).pow(2).mean()
                loss = -surr.mean() + args.value_coef * value_loss - args.entropy * dist.entropy().mean()
                opt.zero_grad()
                loss.backward()
                nn.utils.clip_grad_norm_(net.parameters(), 0.5)
                opt.step()
                losses.append(float(loss.detach()))
        elapsed = time.perf_counter() - started
        print(
            f"update {update:4d}  episodes {episodes:5d}  win {wins / max(episodes, 1):.3f}"
            f"  loss {np.mean(losses):+.3f}  {update * args.steps * args.envs / elapsed:,.0f} steps/s",
            flush=True,
        )
        if update % args.eval_every == 0 or update == args.updates:
            result = evaluate(net, device, args.eval_episodes)
            print(
                f"  eval {result['episodes']} episodes  win {result['win_rate']:.3f}"
                f"  {result['steps_per_s']:,.0f} steps/s",
                flush=True,
            )
            if result["win_rate"] > best:
                best = result["win_rate"]
                pathlib.Path(args.out).parent.mkdir(parents=True, exist_ok=True)
                torch.save(
                    {
                        "model": net.state_dict(),
                        "obs_dim": obs_dim,
                        "actions": actions,
                        "vocab_hash": int(env.lib.sts2_vocab_hash()),
                        "build_id": env.lib.sts2_build_id().decode(),
                        "eval_win_rate": result["win_rate"],
                    },
                    args.out,
                )
                print(f"  saved {args.out}", flush=True)
    env.close()
    return net, best


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--updates", type=int, default=100)
    parser.add_argument("--steps", type=int, default=128, help="env steps per update")
    parser.add_argument("--envs", type=int, default=64)
    parser.add_argument("--hidden", type=int, default=256)
    parser.add_argument("--seed", type=int, default=0)
    parser.add_argument("--lr", type=float, default=3e-4)
    parser.add_argument("--gamma", type=float, default=0.99)
    parser.add_argument("--lam", type=float, default=0.95)
    parser.add_argument("--clip", type=float, default=0.2)
    parser.add_argument("--epochs", type=int, default=4)
    parser.add_argument("--minibatches", type=int, default=4)
    parser.add_argument("--entropy", type=float, default=0.01)
    parser.add_argument("--value-coef", type=float, default=0.5)
    parser.add_argument("--eval-every", type=int, default=10)
    parser.add_argument("--eval-episodes", type=int, default=512)
    parser.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    parser.add_argument("--out", default="target/combat-ppo.pt")
    args = parser.parse_args()
    baseline = evaluate(None, torch.device(args.device), args.eval_episodes,
                        random_baseline=True)
    print(f"random baseline: {baseline['win_rate']:.3f} over {baseline['episodes']} episodes")
    _, best = train(args)
    print(f"best eval win rate: {best:.3f}")


if __name__ == "__main__":
    main()
