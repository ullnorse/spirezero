"""Whole-run evaluation with the learned combat policy and a random run policy (RLB5 probe).

The run env is in agent mode; at a combat decision the trained combat net picks the action from
the run observation's combat section, and at a run-level decision a uniform legal action is
taken. This is not the run policy yet: it measures whether the combat learner transfers to the
states a whole run reaches.

    .venv/bin/python python/eval_combat_policy.py --checkpoint target/combat-ppo.pt --seeds 100
"""

import argparse
import pathlib
import sys

import numpy as np
import torch

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

from sts2 import ffi  # noqa: E402
from sts2.env import CHARACTERS, RUN_ENDS, Sts2RunEnvs  # noqa: E402
from train_combat_ppo import Net, masked_logits  # noqa: E402


def action_from_mask(mask, rng=None):
    """A uniform legal index per row, or 0 for an empty one."""
    if rng is None:
        return np.argmax(mask, axis=1).astype(np.uint16)
    scores = np.where(mask, rng.random(mask.shape), -1.0)
    return np.argmax(scores, axis=1).astype(np.uint16)


def evaluate(character, seeds, checkpoint, envs_n, device, ascension):
    lib = ffi.load()
    layout = ffi.layout(lib)
    combat_obs = int(layout.obs_bytes)
    static_bytes = int(layout.static_bytes)
    run_base = int(ffi.run_layout(lib).run_base)
    run_layout = ffi.run_layout(lib)
    combat_at = int(run_layout.combat_at)
    static_at = int(run_layout.combat_static_at)

    saved = torch.load(checkpoint, weights_only=False)
    net = Net(int(saved["obs_dim"]), int(saved["actions"]), 256).to(device)
    net.load_state_dict(saved["model"])
    net.eval()

    env = Sts2RunEnvs(envs_n, seeds=list(seeds), character=character, ascension=ascension,
                      combat="agent", lib=lib)
    rng = np.random.default_rng(seeds[0])
    done_count = wins = 0
    floors = []
    ends = [0] * len(RUN_ENDS)
    steps = 0
    idle = np.zeros(envs_n, dtype=np.uint16)
    try:
        while done_count < len(seeds):
            legal = env.legal()
            live = legal.any(axis=1)
            if live.any():
                phase = env.obs[:, 3]
                in_combat = phase == 3
                actions = np.zeros(envs_n, dtype=np.uint16)
                run_actions = action_from_mask(legal[:, run_base:], rng) + run_base
                actions[~in_combat] = run_actions[~in_combat]
                if in_combat.any():
                    rows = np.flatnonzero(in_combat)
                    raw = np.concatenate(
                        (
                            env.obs[rows, combat_at:combat_at + combat_obs],
                            env.obs[rows, static_at:static_at + static_bytes],
                        ),
                        axis=1,
                    ).astype(np.float32) / 255.0
                    with torch.inference_mode():
                        logits, _ = net(torch.from_numpy(raw).to(device))
                        combat_actions = torch.argmax(
                            masked_logits(
                                logits,
                                torch.from_numpy(legal[rows, :run_base]).to(device),
                            ),
                            dim=-1,
                        ).cpu().numpy().astype(np.uint16)
                    actions[in_combat] = combat_actions
            else:
                actions = idle
            env.step(actions)
            steps += 1
            done = env.done.astype(bool)
            if done.any():
                done_count += int(done.sum())
                wins += int(env.won[done].sum())
                floors.extend(int(f) for f in env.floor[done])
                for end in env.end[done]:
                    ends[int(end)] += 1
    finally:
        env.close()
    return {
        "character": character,
        "episodes": done_count,
        "wins": wins,
        "win_rate": wins / max(done_count, 1),
        "median_floor": float(np.median(floors)) if floors else 0.0,
        "mean_floor": float(np.mean(floors)) if floors else 0.0,
        "ends": dict(zip(RUN_ENDS, ends)),
        "steps": steps,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkpoint", default="target/combat-ppo.pt")
    parser.add_argument("--seeds", type=int, default=100)
    parser.add_argument("--ascension", type=int, default=10)
    parser.add_argument("--envs", type=int, default=16)
    parser.add_argument("--characters", default=",".join(CHARACTERS))
    parser.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    args = parser.parse_args()
    device = torch.device(args.device)
    seeds = list(range(9_000_000, 9_000_000 + args.seeds))
    for character in [c for c in args.characters.split(",") if c]:
        result = evaluate(character, seeds, args.checkpoint, args.envs, device, args.ascension)
        print(
            f"{character:11s} eps {result['episodes']:4d}  win {result['win_rate']:.3f}"
            f"  floor med/mean {result['median_floor']:.1f}/{result['mean_floor']:.1f}"
            f"  steps {result['steps']}  ends {result['ends']}"
        )


if __name__ == "__main__":
    main()
