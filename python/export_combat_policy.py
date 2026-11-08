"""Exports the combat PPO checkpoint for the Rust honest bot (`sts2-diff bot`; task RLB6).

The format is a little-endian header, `STS2PPO1`, then `obs_dim`, `hidden`, `actions` (u32) and
the vocabulary hash (u64), followed by the policy's f32 arrays in order: body.0 weight and bias,
body.2 weight and bias, policy weight and bias (weights row-major, `out × in`). The value head is
not needed to act.

    .venv/bin/python python/export_combat_policy.py target/combat-ppo.pt target/combat-ppo.bin
"""

import argparse
import struct

import numpy as np
import torch

MAGIC = b"STS2PPO1"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("checkpoint", nargs="?", default="target/combat-ppo.pt")
    parser.add_argument("out", nargs="?", default="target/combat-ppo.bin")
    args = parser.parse_args()
    saved = torch.load(args.checkpoint, weights_only=False, map_location="cpu")
    model = saved["model"]
    hidden = int(model["body.0.weight"].shape[0])
    with open(args.out, "wb") as f:
        f.write(MAGIC)
        f.write(struct.pack("<IIIQ", int(saved["obs_dim"]), hidden, int(saved["actions"]),
                            int(saved["vocab_hash"])))
        for name in ("body.0.weight", "body.0.bias", "body.2.weight", "body.2.bias",
                     "policy.weight", "policy.bias"):
            f.write(model[name].detach().numpy().astype("<f4").tobytes())
    print(f"wrote {args.out}: obs {saved['obs_dim']} hidden {hidden} actions {saved['actions']}")


if __name__ == "__main__":
    main()
