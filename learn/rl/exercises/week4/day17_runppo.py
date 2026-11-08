"""Day 17: PPO over a run. Run features, an actor-critic over the whole 355-index space, a
floor-based reward against the sparse one, and a short training run on greedy-fought runs
measured by the floor it reaches.

Run:  .venv/bin/python learn/rl/exercises/week4/day17_runppo.py [iterations]
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

from common import vocab  # noqa: E402
from week2.day8_nets import DEVICE  # noqa: E402
from week2.day10_ppo import update  # noqa: E402
from week4.day16_runenv import PHASES, RUN_ACTIONS, make_run_envs, measure_floors  # noqa: E402

N_CARDS = len(vocab("cards"))
N_RELICS = len(vocab("relics"))
DECK_AT, DECK = 32, 80
RELICS_AT, RELICS = 352, 32
REWARDS_AT, REWARDS, REWARD_BYTES, REWARD_CARDS = 1024, 8, 21, 5
N_SCALARS = 10 + len(PHASES)
N_FEATURES = N_SCALARS + 2 * N_CARDS + N_RELICS


def featurise_run(obs):
    """n × N_FEATURES float32. Scalars first: hp/max_hp, gold/500, floor/50, floor_in_act/20,
    deck_size/40, relics/10, belt/6, map_row/16 (0 before the first point), removals/3, a
    constant 1, then a one-hot of the phase (len(PHASES)); then a multi-hot of the deck's card
    kinds (DECK slots of 4 bytes at DECK_AT, kind u16 first; 0 is empty); then a multi-hot of
    the card kinds offered by the rewards screen (REWARDS × REWARD_BYTES at REWARDS_AT: 6
    header bytes then REWARD_CARDS × (kind u16, upgrade u8)); then a multi-hot of the relics
    (RELICS slots of 4 bytes at RELICS_AT, kind u16 first)."""
    raise NotImplementedError  # TODO


class RunActorCritic(nn.Module):
    """features → 256 → 256 (ReLU) → RUN_ACTIONS logits and a value; `forward(x, legal)` as
    day 10's, illegal logits at -1e9."""

    def __init__(self, n_features=N_FEATURES, hidden=256, n_actions=RUN_ACTIONS):
        super().__init__()
        self.body = nn.Sequential(nn.Linear(n_features, hidden), nn.ReLU(), nn.Linear(hidden, hidden), nn.ReLU())
        self.pi = nn.Linear(hidden, n_actions)
        self.v = nn.Linear(hidden, 1)

    def forward(self, x, legal):
        raise NotImplementedError  # TODO


def run_reward(kind, floor_before, floor_after, done, won, scale=0.1):
    """Per env, float32. sparse: +1 on a won run, -1 on any other end, 0 otherwise. floor:
    scale per floor gained this step (floor_after - floor_before, 0 on an ending step since the
    observation is a new run's), plus +1 on a won run and nothing on a death: the floors
    climbed are the whole signal. `floor_*` are the header's floor bytes (u8 at 7)."""
    raise NotImplementedError  # TODO


def rollout(envs, net, T, kind="floor", device=DEVICE):
    """T steps of the run envs sampling from `net`, rewards by `run_reward(kind, ...)`. Returns
    day 10's dict plus `floors`: the final floors of the runs that ended."""
    raise NotImplementedError  # TODO


def train(iterations=30, kind="floor", n_envs=64, T=64, lr=3e-4, seed=0, seeds=(0, 1000), log=None, **kw):
    """PPO over `make_run_envs(n_envs, seeds, "greedy", threads=4)` for `iterations`
    rollout+update rounds (day 10's `update`, gamma 0.995 by default: a run is long). Returns
    (net, history) with `mean_floor` of each rollout's finished runs in the stats."""
    raise NotImplementedError  # TODO


def greedy(net):
    """`policy(obs, legal)` for day 16's `measure_floors`: the net's argmax legal action."""
    def policy(obs, legal):
        with torch.no_grad():
            dist, _ = net(torch.as_tensor(featurise_run(obs), device=DEVICE), torch.as_tensor(legal, device=DEVICE))
        return dist.logits.argmax(dim=1).cpu().numpy()
    return policy


if __name__ == "__main__":
    from week4.day16_runenv import lowest_policy, random_run_policy

    iterations = int(sys.argv[1]) if len(sys.argv) > 1 else 60
    eval_seeds = (5000, 6000)
    for name, policy in (("random", random_run_policy), ("lowest index", lowest_policy)):
        mean, se, _, _ = measure_floors(policy, n_runs=300, seeds=eval_seeds)
        print(f"{name:14s} floor {mean:.2f} ± {se:.2f}")
    net, history = train(20, "sparse")
    print(f"ppo, sparse reward: every run ends -1, the normalised advantages are noise; rollout floor "
          f"{history[0]['mean_floor']:.2f} → {history[-1]['mean_floor']:.2f} over 20 iterations")
    t0 = time.time()
    net, history = train(iterations, "floor", log=lambda it, s: print(f"  it {it:3d} floor {s['mean_floor']:5.2f} ({s['episodes']} runs) entropy {s['entropy']:.2f} kl {s['approx_kl']:.4f}") if it % 10 == 0 else None)
    mean, se, floors, ends = measure_floors(greedy(net), n_runs=300, seeds=eval_seeds)
    print(f"ppo, floor reward: greedy floor {mean:.2f} ± {se:.2f} over {len(floors)} runs after {iterations} iterations, {time.time() - t0:.0f}s; ends {ends}")
