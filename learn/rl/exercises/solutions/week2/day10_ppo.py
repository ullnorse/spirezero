"""Day 10: PPO on the real env. GAE with done cuts, the clipped objective, masked logits, and
a short training run. Reference solution."""

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
from solutions.week2.day8_nets import DEVICE, N_FEATURES, featurise  # noqa: E402


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
        h = self.body(x)
        logits = self.pi(h).masked_fill(~legal, -1e9)
        return torch.distributions.Categorical(logits=logits), self.v(h).squeeze(-1)


def gae(rewards, values, dones, last_value, gamma=0.99, lam=0.95):
    """Generalised advantage estimation over a T × n rollout. `values[t]` is V(s_t),
    `last_value` V of the state after step T-1, `dones[t]` 1.0 where step t ended a fight (the
    next observation is a new fight, so the chain is cut there: no bootstrap, no carried
    advantage). Returns (advantages, returns = advantages + values), both T × n."""
    T = len(rewards)
    adv = torch.zeros_like(rewards)
    carry = torch.zeros_like(rewards[0])
    for t in reversed(range(T)):
        nonterminal = 1.0 - dones[t]
        next_value = last_value if t == T - 1 else values[t + 1]
        delta = rewards[t] + gamma * next_value * nonterminal - values[t]
        carry = delta + gamma * lam * nonterminal * carry
        adv[t] = carry
    return adv, adv + values


def ppo_loss(new_logp, old_logp, adv, clip=0.2):
    """The clipped surrogate, to minimise: -mean(min(ratio · adv, clip(ratio, 1-clip, 1+clip)
    · adv)) with ratio = exp(new_logp - old_logp). Also returns (clip fraction, approx KL =
    mean(old_logp - new_logp))."""
    ratio = torch.exp(new_logp - old_logp)
    unclipped = ratio * adv
    clipped = torch.clamp(ratio, 1 - clip, 1 + clip) * adv
    loss = -torch.min(unclipped, clipped).mean()
    clipfrac = float(((ratio - 1).abs() > clip).float().mean())
    approx_kl = float((old_logp - new_logp).detach().mean())
    return loss, clipfrac, approx_kl


def rollout(envs, net, T, device=DEVICE):
    """T steps of `envs` sampling from `net`. Returns a dict of tensors on `device`: x (T × n ×
    F), legal (T × n × 140 bool), act, logp, val, rew, done (T × n), last_val (n), and the
    counts episodes and wins."""
    n = envs.n
    out = {k: [] for k in ("x", "legal", "act", "logp", "val", "rew", "done")}
    episodes = wins = 0
    legal = envs.legal()
    x = torch.as_tensor(featurise(envs.obs, legal), device=device)
    legal_t = torch.as_tensor(legal, device=device)
    for _ in range(T):
        with torch.no_grad():
            dist, val = net(x, legal_t)
            act = dist.sample()
            logp = dist.log_prob(act)
        envs.step(act.cpu().numpy())
        done = envs.done.astype(bool)
        rew = np.where(done, np.where(envs.won == 1, 1.0, -1.0), 0.0).astype(np.float32)
        episodes += int(done.sum())
        wins += int(envs.won.sum())
        for k, v in (("x", x), ("legal", legal_t), ("act", act), ("logp", logp), ("val", val),
                     ("rew", torch.as_tensor(rew, device=device)),
                     ("done", torch.as_tensor(done.astype(np.float32), device=device))):
            out[k].append(v)
        legal = envs.legal()
        x = torch.as_tensor(featurise(envs.obs, legal), device=device)
        legal_t = torch.as_tensor(legal, device=device)
    batch = {k: torch.stack(v) for k, v in out.items()}
    with torch.no_grad():
        _, batch["last_val"] = net(x, legal_t)
    batch["episodes"], batch["wins"] = episodes, wins
    return batch


def update(net, opt, batch, epochs=4, minibatch=1024, clip=0.2, vf_coef=0.5, ent_coef=0.01, gamma=0.99,
           lam=0.95, max_grad=0.5):
    """PPO update on one rollout: advantages by `gae`, normalised; `epochs` passes over
    shuffled minibatches of the flattened T × n steps minimising policy loss + vf_coef ·
    value MSE - ent_coef · entropy. Returns a dict of means: loss, value_loss, entropy,
    clipfrac, approx_kl."""
    adv, ret = gae(batch["rew"], batch["val"], batch["done"], batch["last_val"], gamma, lam)
    adv = (adv - adv.mean()) / (adv.std() + 1e-8)
    T, n = batch["rew"].shape
    flat = {k: batch[k].reshape(T * n, *batch[k].shape[2:]) for k in ("x", "legal", "act", "logp")}
    adv, ret = adv.reshape(-1), ret.reshape(-1)
    stats = {k: [] for k in ("loss", "value_loss", "entropy", "clipfrac", "approx_kl")}
    for _ in range(epochs):
        perm = torch.randperm(T * n, device=adv.device)
        for start in range(0, T * n, minibatch):
            idx = perm[start:start + minibatch]
            dist, val = net(flat["x"][idx], flat["legal"][idx])
            pl, clipfrac, kl = ppo_loss(dist.log_prob(flat["act"][idx]), flat["logp"][idx], adv[idx], clip)
            vl = ((val - ret[idx]) ** 2).mean()
            ent = dist.entropy().mean()
            loss = pl + vf_coef * vl - ent_coef * ent
            opt.zero_grad()
            loss.backward()
            nn.utils.clip_grad_norm_(net.parameters(), max_grad)
            opt.step()
            for k, v in (("loss", loss.item()), ("value_loss", vl.item()), ("entropy", ent.item()),
                         ("clipfrac", clipfrac), ("approx_kl", kl)):
                stats[k].append(v)
    return {k: float(np.mean(v)) for k, v in stats.items()}


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
    envs = make_envs(n_envs, seed=seed)
    wins = episodes = 0
    for _ in range(n_steps):
        legal = envs.legal()
        with torch.no_grad():
            dist, _ = net(torch.as_tensor(featurise(envs.obs, legal), device=device), torch.as_tensor(legal, device=device))
        envs.step(dist.logits.argmax(dim=1).cpu().numpy())
        episodes += int(envs.done.sum())
        wins += int(envs.won.sum())
    envs.close()
    return wins / max(1, episodes), episodes


if __name__ == "__main__":
    t0 = time.time()
    iterations = int(sys.argv[1]) if len(sys.argv) > 1 else 40

    def log(it, s):
        print(f"it {it:3d}  win {s['win_rate']:.3f} ({s['episodes']} fights)  entropy {s['entropy']:.2f}  "
              f"clip {s['clipfrac']:.2f}  kl {s['approx_kl']:.4f}  vloss {s['value_loss']:.3f}  {time.time() - t0:5.0f}s")
    net, history = train(iterations, log=log)
    wr, n = evaluate(net)
    print(f"greedy win rate {wr:.3f} over {n} fights (random 0.34, cards first 0.58+)")
