"""Day 10: PPO on the real env. GAE with done cuts, the clipped objective, masked logits, and
a short training run.

Run:  .venv/bin/python learn/rl/exercises/week2/day10_ppo.py [iterations]
Test: .venv/bin/python -m pytest learn/rl/exercises/week2/test_day10.py
"""

import pathlib
import sys
import time

ROOT = pathlib.Path(__file__).resolve()
while not (ROOT / "Cargo.toml").exists():
    ROOT = ROOT.parent
sys.path[:0] = [str(ROOT / "python"), str(ROOT / "learn/rl/exercises")]

import numpy as np  # noqa: E402
import torch  # noqa: E402
from torch import nn  # noqa: E402

from common import ACTIONS, make_envs  # noqa: E402
from week2.day8_nets import DEVICE, N_FEATURES, featurise  # noqa: E402


class ActorCritic(nn.Module):
    """features → 256 → 256 (ReLU) → 140 logits and a value. `forward(x, legal)` returns
    (Categorical over the legal actions, value), masking illegal logits with -1e9."""

    def __init__(self, n_features=N_FEATURES, hidden=256, n_actions=ACTIONS):
        super().__init__()
        self.body = nn.Sequential(nn.Linear(n_features, hidden), nn.ReLU(), nn.Linear(hidden, hidden), nn.ReLU())
        self.pi = nn.Linear(hidden, n_actions)
        self.v = nn.Linear(hidden, 1)

    def forward(self, x, legal):
        """(Categorical over legal actions, value) for a batch; `legal` is a bool tensor."""
        raise NotImplementedError  # TODO


def gae(rewards, values, dones, last_value, gamma=0.99, lam=0.95):
    """Generalised advantage estimation over a T × n rollout. `values[t]` is V(s_t),
    `last_value` V of the state after step T-1, `dones[t]` 1.0 where step t ended a fight (the
    next observation is a new fight, so the chain is cut there: no bootstrap, no carried
    advantage). Returns (advantages, returns = advantages + values), both T × n."""
    raise NotImplementedError  # TODO


def ppo_loss(new_logp, old_logp, adv, clip=0.2):
    """The clipped surrogate, to minimise: -mean(min(ratio · adv, clip(ratio, 1-clip, 1+clip)
    · adv)) with ratio = exp(new_logp - old_logp). Also returns (clip fraction, approx KL =
    mean(old_logp - new_logp))."""
    raise NotImplementedError  # TODO


def rollout(envs, net, T, device=DEVICE):
    """T steps of `envs` sampling from `net`. Returns a dict of tensors on `device`: x (T × n ×
    F), legal (T × n × 140 bool), act, logp, val, rew, done (T × n), last_val (n), and the
    counts episodes and wins."""
    raise NotImplementedError  # TODO


def update(net, opt, batch, epochs=4, minibatch=1024, clip=0.2, vf_coef=0.5, ent_coef=0.01, gamma=0.99,
           lam=0.95, max_grad=0.5):
    """PPO update on one rollout: advantages by `gae`, normalised; `epochs` passes over
    shuffled minibatches of the flattened T × n steps minimising policy loss + vf_coef ·
    value MSE - ent_coef · entropy. Returns a dict of means: loss, value_loss, entropy,
    clipfrac, approx_kl."""
    raise NotImplementedError  # TODO


def train(iterations=20, n_envs=64, T=128, lr=3e-4, seed=0, device=DEVICE, log=None, **kw):
    """`iterations` rollout+update rounds on `make_envs(n_envs, seed)`. Returns (net, history:
    one dict per iteration with the update stats plus win_rate and episodes of its rollout)."""
    torch.manual_seed(seed)
    envs = make_envs(n_envs, seed=seed)
    net = ActorCritic().to(device)
    opt = torch.optim.Adam(net.parameters(), lr=lr, eps=1e-5)
    history = []
    for it in range(iterations):
        batch = rollout(envs, net, T, device)
        stats = update(net, opt, batch, **kw)
        stats["win_rate"] = batch["wins"] / max(1, batch["episodes"])
        stats["episodes"] = batch["episodes"]
        history.append(stats)
        if log:
            log(it, stats)
    envs.close()
    return net, history


def evaluate(net, n_envs=64, n_steps=1000, seed=7, device=DEVICE):
    """Greedy (argmax) win rate and fights of `net` on fresh envs."""
    raise NotImplementedError  # TODO


if __name__ == "__main__":
    t0 = time.time()
    iterations = int(sys.argv[1]) if len(sys.argv) > 1 else 40

    def log(it, s):
        print(f"it {it:3d}  win {s['win_rate']:.3f} ({s['episodes']} fights)  entropy {s['entropy']:.2f}  "
              f"clip {s['clipfrac']:.2f}  kl {s['approx_kl']:.4f}  vloss {s['value_loss']:.3f}  {time.time() - t0:5.0f}s")
    net, history = train(iterations, log=log)
    wr, n = evaluate(net)
    print(f"greedy win rate {wr:.3f} over {n} fights (random 0.34, cards first 0.58+)")
