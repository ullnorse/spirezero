"""Day 14: features that see the hand. A card embedding per hand slot and a policy head aligned
with the action layout (play index = slot × 8 + target), trained with day 10's PPO and compared
with day 10's net on the same seeds. Reference solution.

Run:  .venv/bin/python learn/rl/exercises/week3/day14_features.py [iterations]
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

from common import ACTIONS, CARD_BYTES, CARDS_AT, HAND, HAND_AT, POTION_BASE, TARGET_SLOTS, make_envs  # noqa: E402
from solutions.week2.day8_nets import DEVICE, N_CARDS, N_SCALARS, featurise  # noqa: E402
from solutions.week2.day10_ppo import ActorCritic, update  # noqa: E402

SLOT_FIELDS = 3  # kind id, upgrade, cost per hand slot
N_FEATURES = N_SCALARS + HAND * SLOT_FIELDS


def hand_slots(obs):
    """Per hand slot (n × 10 × 3 int64): the card's kind id, upgrade level and cost, from the
    cards table (6 bytes per card at CARDS_AT: kind u16, upgrade u8, pile u8, cost i8, flags)
    indexed by the hand's card ids (u16 at HAND_AT); zeros for empty slots (hand size is
    header byte 16)."""
    obs = np.asarray(obs, dtype=np.uint8)
    n = len(obs)
    table = obs[:, CARDS_AT:CARDS_AT + 64 * CARD_BYTES].reshape(n, 64, CARD_BYTES)
    kinds = table[:, :, 0:2].copy().view("<u2")[:, :, 0].astype(np.int64)
    upgrade = table[:, :, 2].astype(np.int64)
    cost = table[:, :, 4].astype(np.int8).astype(np.int64)
    ids = obs[:, HAND_AT:HAND_AT + HAND * 2].copy().view("<u2").astype(np.int64)
    size = obs[:, 16].astype(np.int64)
    rows = np.arange(n)[:, None]
    out = np.stack([kinds[rows, ids], upgrade[rows, ids], cost[rows, ids]], axis=2)
    out[np.arange(HAND)[None, :] >= size[:, None]] = 0
    return out


def featurise14(obs, legal):
    """n × N_FEATURES float32: day 8's N_SCALARS scalars, then for each of the 10 hand slots
    (kind id, upgrade, cost) as floats, slot-major. The kind id is an index for an embedding,
    not a magnitude; the net reads it back with `.long()`."""
    scalars = featurise(obs, legal)[:, :N_SCALARS]
    return np.concatenate([scalars, hand_slots(obs).reshape(len(obs), -1).astype(np.float32)], axis=1)


class SlotNet(nn.Module):
    """A card embedding (N_CARDS × dim) per hand slot. The body sees the scalars and the mean
    of the hand's embeddings (empty slots excluded), hidden units each. Play logits for slot s
    come from a per-slot head over [embedding(kind_s), upgrade_s, cost_s / 3, body output]:
    8 target logits per slot, laid out at index s × 8 + t. The other 60 logits (potions, End
    turn, picks, Done) and the value come from the body output. `forward(x, legal)` returns
    (Categorical over legal actions, value); day 10's `update` and `rollout` shape."""

    def __init__(self, n_cards=N_CARDS, dim=32, hidden=256, n_actions=ACTIONS):
        super().__init__()
        self.embed = nn.Embedding(n_cards, dim)
        self.body = nn.Sequential(nn.Linear(N_SCALARS + dim, hidden), nn.ReLU(), nn.Linear(hidden, hidden), nn.ReLU())
        self.slot = nn.Sequential(nn.Linear(dim + 2 + hidden, 128), nn.ReLU(), nn.Linear(128, TARGET_SLOTS))
        self.other = nn.Linear(hidden, n_actions - POTION_BASE)
        self.v = nn.Linear(hidden, 1)

    def forward(self, x, legal):
        scalars = x[:, :N_SCALARS]
        slots = x[:, N_SCALARS:].reshape(len(x), HAND, SLOT_FIELDS)
        kinds = slots[:, :, 0].long()
        e = self.embed(kinds)                                            # n × 10 × dim
        present = (kinds > 0).float().unsqueeze(-1)
        pooled = (e * present).sum(1) / present.sum(1).clamp(min=1.0)
        h = self.body(torch.cat([scalars, pooled], dim=1))
        per_slot = torch.cat([e, slots[:, :, 1:2], slots[:, :, 2:3] / 3.0, h.unsqueeze(1).expand(-1, HAND, -1)], dim=2)
        play = self.slot(per_slot).reshape(len(x), HAND * TARGET_SLOTS)
        logits = torch.cat([play, self.other(h)], dim=1).masked_fill(~legal, -1e9)
        return torch.distributions.Categorical(logits=logits), self.v(h).squeeze(-1)


def permutation_test(net, x, legal, a, b):
    """Swaps hand slots `a` and `b` in `x` (features and legal bits) and returns (max absolute
    difference between the swapped play logits and the originals, max absolute difference of
    the non-play logits), both over the batch, without gradients. For a slot-aligned net both
    are 0 up to float error."""
    def swapped(t, width):
        t = t.clone()
        ia, ib = slice(N_SCALARS + a * width, N_SCALARS + (a + 1) * width), slice(N_SCALARS + b * width, N_SCALARS + (b + 1) * width)
        t[:, ia], t[:, ib] = t[:, ib].clone(), t[:, ia].clone()
        return t
    x2 = swapped(x, SLOT_FIELDS)
    legal2 = legal.clone()
    pa, pb = slice(a * TARGET_SLOTS, (a + 1) * TARGET_SLOTS), slice(b * TARGET_SLOTS, (b + 1) * TARGET_SLOTS)
    legal2[:, pa], legal2[:, pb] = legal[:, pb], legal[:, pa]
    with torch.no_grad():
        l1 = net(x, legal)[0].logits
        l2 = net(x2, legal2)[0].logits
    play_diff = max(float((l1[:, pa] - l2[:, pb]).abs().max()), float((l1[:, pb] - l2[:, pa]).abs().max()))
    other_diff = float((l1[:, POTION_BASE:] - l2[:, POTION_BASE:]).abs().max())
    return play_diff, other_diff


def rollout(envs, net, T, featurise_fn, device=DEVICE):
    """Day 10's rollout with the featuriser as an argument: the same dict of tensors."""
    n = envs.n
    out = {k: [] for k in ("x", "legal", "act", "logp", "val", "rew", "done")}
    episodes = wins = 0
    legal = envs.legal()
    x = torch.as_tensor(featurise_fn(envs.obs, legal), device=device)
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
        x = torch.as_tensor(featurise_fn(envs.obs, legal), device=device)
        legal_t = torch.as_tensor(legal, device=device)
    batch = {k: torch.stack(v) for k, v in out.items()}
    with torch.no_grad():
        _, batch["last_val"] = net(x, legal_t)
    batch["episodes"], batch["wins"] = episodes, wins
    return batch


def train(net, featurise_fn, iterations=20, n_envs=64, T=128, lr=3e-4, seed=0, log=None, **kw):
    """Day 10's loop for any (net, featuriser) pair. Returns (net, history) as day 10's."""
    torch.manual_seed(seed)
    envs = make_envs(n_envs, seed=seed)
    net = net.to(DEVICE)
    opt = torch.optim.Adam(net.parameters(), lr=lr, eps=1e-5)
    history = []
    for it in range(iterations):
        batch = rollout(envs, net, T, featurise_fn)
        stats = update(net, opt, batch, **kw)
        stats["win_rate"] = batch["wins"] / max(1, batch["episodes"])
        stats["episodes"] = batch["episodes"]
        history.append(stats)
        if log:
            log(it, stats)
    envs.close()
    return net, history


def evaluate(net, featurise_fn, n_fights=512, n_envs=64, seed=7):
    """Day 13's protocol for any featuriser: greedy win rate over the first `n_fights` fights of
    `make_envs(n_envs, seed)`. Returns (win_rate, fights)."""
    envs = make_envs(n_envs, seed=seed)
    wins = fights = 0
    while fights < n_fights:
        legal = envs.legal()
        with torch.no_grad():
            dist, _ = net(torch.as_tensor(featurise_fn(envs.obs, legal), device=DEVICE), torch.as_tensor(legal, device=DEVICE))
        envs.step(dist.logits.argmax(dim=1).cpu().numpy())
        for i in np.flatnonzero(envs.done):
            if fights < n_fights:
                fights += 1
                wins += int(envs.won[i])
    envs.close()
    return wins / fights, fights


if __name__ == "__main__":
    iterations = int(sys.argv[1]) if len(sys.argv) > 1 else 40
    for name, net, f in (("day 10 multi-hot", ActorCritic(), featurise), ("slot net", SlotNet(), featurise14)):
        t0 = time.time()
        net, history = train(net, f, iterations)
        wr, n = evaluate(net, f)
        print(f"{name:18s} after {iterations} iterations: training win {history[-1]['win_rate']:.3f}, greedy eval {wr:.3f} over {n} fights, "
              f"entropy {history[-1]['entropy']:.2f}, {time.time() - t0:.0f}s")
