"""Writes the value net's parity fixture (RLB11): raw observations from random play on a start
corpus and the PyTorch net's outputs on them, for `honest/value.rs`'s test.

The file is `count` (u32), then per row the raw observation (OBS_BYTES + STATIC_BYTES bytes),
the win and HP probabilities and the 140 logits (f32, little-endian).

    .venv/bin/python python/rlb11_fixture.py CHECKPOINT.pt STARTS.jsonl OUT.bin
"""

import struct
import sys

import numpy as np
import torch

sys.path.insert(0, __file__.rsplit("/", 1)[0])

from sts2.combat_net import Net  # noqa: E402
from sts2.env import Sts2PufferEnv  # noqa: E402


def main():
    ckpt, starts, out = sys.argv[1:4]
    saved = torch.load(ckpt, map_location="cpu", weights_only=False)
    net = Net(saved["hidden"])
    net.load_state_dict(saved["model"])
    env = Sts2PufferEnv(16, starts=starts, seed=5, with_static=True)
    env.reset()
    rng = np.random.default_rng(1)
    rows = []
    for step in range(40):
        if step % 10 == 0:
            rows.append(env.observations.copy())
        legal = env.action_masks
        env.step(np.argmax(np.where(legal, rng.random(legal.shape), -1), axis=1))
    raw = np.concatenate(rows)
    with torch.no_grad():
        logits, win, hp = net(torch.from_numpy(raw))
    with open(out, "wb") as f:
        f.write(struct.pack("<I", raw.shape[0]))
        for i in range(raw.shape[0]):
            f.write(raw[i].tobytes())
            f.write(struct.pack("<ff", torch.sigmoid(win[i]).item(), torch.sigmoid(hp[i]).item()))
            f.write(logits[i].numpy().astype("<f4").tobytes())
    print(f"wrote {raw.shape[0]} rows to {out}")


if __name__ == "__main__":
    main()
