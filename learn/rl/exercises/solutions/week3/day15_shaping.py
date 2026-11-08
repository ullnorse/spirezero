"""Day 15: reward shaping on the real env. Four reward functions over the same rollout data,
the telescoping check for the potential-based one, and PPO trained with each on the same seeds
and judged by the greedy win rate and the HP left after wins. Reference solution.

Run:  .venv/bin/python learn/rl/exercises/week3/day15_shaping.py [iterations]
"""

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve()
while not (ROOT / "Cargo.toml").exists():
    ROOT = ROOT.parent
sys.path[:0] = [str(ROOT / "python"), str(ROOT / "learn/rl/exercises")]

import numpy as np  # noqa: E402
import torch  # noqa: E402

from common import make_envs  # noqa: E402
from solutions.week2.day8_nets import DEVICE, featurise  # noqa: E402
from solutions.week2.day10_ppo import ActorCritic, update  # noqa: E402

KINDS = ("sparse", "hp_loss", "win_hp_bonus", "potential")


def hp_fraction(obs):
    """HP / max HP per env from the header (i16 at bytes 0 and 2), as float64; 0 for a max HP
    of 0."""
    h = np.asarray(obs, dtype=np.uint8)[:, 0:4].copy().view("<i2")
    return np.where(h[:, 1] > 0, h[:, 0] / np.maximum(h[:, 1], 1), 0.0)


def shaped(kind, obs_before, obs_after, done, won, scale=0.1, gamma=0.99):
    """One step's reward per env, by `kind`:
    sparse: +1 won, -1 other end, 0 otherwise;
    hp_loss: sparse - scale · max(0, hp fraction lost this step); 0 lost on an ending step
    (the next observation is a new fight's);
    win_hp_bonus: sparse + scale · hp fraction of obs_before on a step that won (the HP the
    fight ended with is the HP before the killing blow);
    potential: sparse + gamma · Φ(s') - Φ(s) with Φ = scale · hp fraction, and Φ(s') = 0 on an
    ending step (the terminal state's potential is zero).
    `done` and `won` are uint8 arrays; returns float32."""
    done = np.asarray(done, dtype=bool)
    won = np.asarray(won, dtype=bool)
    r = np.where(done, np.where(won, 1.0, -1.0), 0.0)
    before, after = hp_fraction(obs_before), hp_fraction(obs_after)
    if kind == "sparse":
        pass
    elif kind == "hp_loss":
        r = r - scale * np.where(done, 0.0, np.maximum(0.0, before - after))
    elif kind == "win_hp_bonus":
        r = r + scale * np.where(done & won, before, 0.0)
    elif kind == "potential":
        r = r + gamma * np.where(done, 0.0, scale * after) - scale * before
    else:
        raise ValueError(kind)
    return r.astype(np.float32)


def telescoped(phis, gamma=0.99):
    """What a potential-based term adds to the discounted return of one episode: for the
    sequence Φ(s_0) ... Φ(s_{T-1}) (the last state before the terminal one, whose Φ is 0),
    Σ_t γ^t (γ Φ(s_{t+1}) - Φ(s_t)), which telescopes to -Φ(s_0). Returns the sum, computed
    term by term so the test can check the claim."""
    phis = list(phis) + [0.0]
    return float(sum(gamma ** t * (gamma * phis[t + 1] - phis[t]) for t in range(len(phis) - 1)))


def rollout(envs, net, T, kind, device=DEVICE, scale=0.1, gamma=0.99):
    """Day 10's rollout with `shaped(kind, ...)` as the reward: the observation before and
    after each step are both needed, so copy `envs.obs` before stepping. Also counts
    `hp_won`: the sum of hp fractions (before the ending step) over the fights won."""
    n = envs.n
    out = {k: [] for k in ("x", "legal", "act", "logp", "val", "rew", "done")}
    episodes = wins = 0
    hp_won = 0.0
    legal = envs.legal()
    x = torch.as_tensor(featurise(envs.obs, legal), device=device)
    legal_t = torch.as_tensor(legal, device=device)
    for _ in range(T):
        with torch.no_grad():
            dist, val = net(x, legal_t)
            act = dist.sample()
            logp = dist.log_prob(act)
        before = envs.obs.copy()
        envs.step(act.cpu().numpy())
        done = envs.done.astype(bool)
        rew = shaped(kind, before, envs.obs, envs.done, envs.won, scale, gamma)
        episodes += int(done.sum())
        wins += int(envs.won.sum())
        hp_won += float(hp_fraction(before)[done & (envs.won == 1)].sum())
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
    batch["episodes"], batch["wins"], batch["hp_won"] = episodes, wins, hp_won
    return batch


def train(kind, iterations=20, n_envs=64, T=128, lr=3e-4, seed=0, scale=0.1, log=None, **kw):
    """PPO with reward `kind`, otherwise day 10's `train`. Returns (net, history)."""
    torch.manual_seed(seed)
    envs = make_envs(n_envs, seed=seed)
    net = ActorCritic().to(DEVICE)
    opt = torch.optim.Adam(net.parameters(), lr=lr, eps=1e-5)
    history = []
    for it in range(iterations):
        batch = rollout(envs, net, T, kind, scale=scale)
        stats = update(net, opt, batch, **kw)
        stats["win_rate"] = batch["wins"] / max(1, batch["episodes"])
        stats["episodes"] = batch["episodes"]
        history.append(stats)
        if log:
            log(it, stats)
    envs.close()
    return net, history


def evaluate(net, n_fights=512, n_envs=64, seed=7):
    """Greedy play over the first `n_fights` fights of `make_envs(n_envs, seed)`. Returns
    (win_rate, mean hp fraction left after the won fights, mean fight length): the numbers the
    reward functions are judged by, which none of them is."""
    envs = make_envs(n_envs, seed=seed)
    length = np.zeros(n_envs, dtype=np.int64)
    wins = fights = total_len = 0
    hp = 0.0
    while fights < n_fights:
        legal = envs.legal()
        with torch.no_grad():
            dist, _ = net(torch.as_tensor(featurise(envs.obs, legal), device=DEVICE), torch.as_tensor(legal, device=DEVICE))
        before = hp_fraction(envs.obs)
        envs.step(dist.logits.argmax(dim=1).cpu().numpy())
        length += 1
        for i in np.flatnonzero(envs.done):
            if fights >= n_fights:
                break
            fights += 1
            total_len += int(length[i])
            if envs.won[i]:
                wins += 1
                hp += float(before[i])
            length[i] = 0
        length[envs.done.astype(bool)] = 0
    envs.close()
    return wins / fights, hp / max(1, wins), total_len / fights


if __name__ == "__main__":
    iterations = int(sys.argv[1]) if len(sys.argv) > 1 else 40
    print(f"potential term over an episode with Φ = 0.1 · (1.0, 0.8, 0.5): {telescoped([0.1, 0.08, 0.05]):+.4f} (-Φ(s_0) = -0.1)")
    for kind in KINDS:
        net, history = train(kind, iterations)
        wr, hp, length = evaluate(net)
        print(f"{kind:13s} greedy win {wr:.3f}  HP left after wins {hp:.2f}  length {length:.1f}  "
              f"(training win {history[-1]['win_rate']:.3f}, entropy {history[-1]['entropy']:.2f})")
