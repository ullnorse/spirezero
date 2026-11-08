"""Day 12: why REINFORCE stalled on the lethal block, and the three fixes: a feature that makes
the decision linear, a learned baseline (a critic), an entropy bonus. Mini fight, CPU.
Reference solution.

Run:  .venv/bin/python learn/rl/exercises/week3/day12_baseline.py
"""

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve()
while not (ROOT / "Cargo.toml").exists():
    ROOT = ROOT.parent
sys.path[:0] = [str(ROOT / "python"), str(ROOT / "learn/rl/exercises")]

import numpy as np  # noqa: E402
import torch  # noqa: E402
from torch import nn  # noqa: E402

from common.minifight import STRIKE_DMG, MiniFight, State, win_rate  # noqa: E402
from solutions.week2.day9_reinforce import PROBE, features, masked_dist, returns  # noqa: E402

N_FEATURES = 13
# Worm at 7 with a lethal Chomp coming: Defend survives and leaves the kill for next turn.
PROBE_WIN = State(player_hp=7, block=5, energy=1, enemy_hp=7, intent="chomp", strikes=1, defends=1, draw_s=2, draw_d=3,
                  disc_s=2, disc_d=1, turn=2)


def features13(s):
    """Day 9's 11 features plus two bits: lethal (the damage that gets through the block is at
    least the HP left) and kill (the enemy's HP minus block is within one Strike)."""
    through = max(0, s.incoming() - s.block)
    return np.concatenate([features(s), [1.0 if through >= s.player_hp else 0.0,
                                         1.0 if s.enemy_hp - s.enemy_block <= STRIKE_DMG else 0.0]]).astype(np.float32)


class ActorCritic(nn.Module):
    """features → 64 (tanh) shared, then 3 logits and one value. `forward(x)` returns (logits,
    value) with the value squeezed to the batch shape."""

    def __init__(self, n_features=N_FEATURES, hidden=64):
        super().__init__()
        self.body = nn.Sequential(nn.Linear(n_features, hidden), nn.Tanh())
        self.pi = nn.Linear(hidden, 3)
        self.v = nn.Linear(hidden, 1)

    def forward(self, x):
        h = self.body(x)
        return self.pi(h), self.v(h).squeeze(-1)


def collect(fight, net, n_episodes, feats=features13, gamma=0.99):
    """Plays `n_episodes` fights sampling from `net`. Returns (X: steps × F, actions: steps,
    legal: steps × 3 bool, G: steps returns by `returns`, wins) with every fight's steps
    concatenated in play order; no gradients are kept."""
    xs, acts, legs, gs, wins = [], [], [], [], 0
    for _ in range(n_episodes):
        s = fight.reset()
        rews = []
        while not s.done:
            x = torch.as_tensor(feats(s))
            legal = torch.tensor(fight.legal(s))
            with torch.no_grad():
                logits, _ = net(x)
            a = int(masked_dist(logits, legal).sample())
            r, _ = fight.step(s, a)
            xs.append(x)
            acts.append(a)
            legs.append(legal)
            rews.append(float(r))
        gs.extend(returns(rews, gamma))
        wins += s.won
    return torch.stack(xs), torch.tensor(acts), torch.stack(legs), torch.tensor(gs, dtype=torch.float32), wins


def actor_critic_loss(logits, values, legal, actions, G, ent_coef=0.03, vf_coef=0.5, use_critic=True):
    """Policy loss -mean(log π(a|s) · A) with A = G - V(s) (V detached) when `use_critic`, else
    A = G; plus vf_coef · mean((V - G)²) when `use_critic` (without a critic the value head is
    left untrained, so it cannot disturb the shared body); minus ent_coef · mean entropy of the
    masked distribution. Returns (loss, policy_loss, value_loss, entropy) as tensors."""
    dist = masked_dist(logits, legal)
    adv = G - values.detach() if use_critic else G
    pg = -(dist.log_prob(actions) * adv).mean()
    vl = ((values - G) ** 2).mean()
    ent = dist.entropy().mean()
    loss = pg - ent_coef * ent
    if use_critic:
        loss = loss + vf_coef * vl
    return loss, pg, vl, ent


def train(episodes=3000, batch=25, lr=0.01, gamma=0.99, ent_coef=0.03, use_critic=True, feats=features13, seed=0,
          window=250):
    """Every `batch` episodes one Adam step on `actor_critic_loss` over their steps. Returns
    (net, training win rate per `window` episodes; `window` is a multiple of `batch`)."""
    torch.manual_seed(seed)
    fight = MiniFight(seed)
    net = ActorCritic(len(feats(State())))
    opt = torch.optim.Adam(net.parameters(), lr=lr)
    curve, wins = [], 0
    for ep in range(0, episodes, batch):
        X, A, L, G, w = collect(fight, net, batch, feats, gamma)
        wins += w
        logits, values = net(X)
        loss, _, _, _ = actor_critic_loss(logits, values, L, A, G, ent_coef, use_critic=use_critic)
        opt.zero_grad()
        loss.backward()
        opt.step()
        if (ep + batch) % window == 0:
            curve.append(wins / window)
            wins = 0
    return net, curve


def greedy_policy(net, feats=features13):
    """`policy(state, legal, rng)` that plays the net's most likely legal action."""
    def policy(s, legal, rng):
        with torch.no_grad():
            logits, _ = net(torch.as_tensor(feats(s)))
        return int(logits.masked_fill(~torch.tensor(legal), -1e9).argmax())
    return policy


def p_defend(net, s=PROBE, feats=features13):
    """P(Defend | s) under the net."""
    with torch.no_grad():
        logits, _ = net(torch.as_tensor(feats(s)))
    return float(masked_dist(logits, torch.tensor(MiniFight.legal(s))).probs[1])


if __name__ == "__main__":
    for name, kw in (("day 9 features, no critic, no entropy", dict(feats=features, use_critic=False, ent_coef=0.0)),
                     ("day 9 features, critic + entropy", dict(feats=features)),
                     ("13 features, no critic, no entropy", dict(use_critic=False, ent_coef=0.0)),
                     ("13 features, critic + entropy", dict())):
        net, curve = train(**kw)
        f = kw.get("feats", features13)
        print(f"{name:38s} training {curve[0]:.2f} → {curve[-1]:.2f}  greedy {win_rate(greedy_policy(net, f), 1000, seed=3):.3f}  "
              f"P(Defend | lethal) {p_defend(net, PROBE, f):.2f}  P(Defend | lethal, kill next turn) {p_defend(net, PROBE_WIN, f):.2f}")
