"""CB2: fork a train_e2e checkpoint into a new lane with changed gym reward settings.

    python tools/cb2_fork.py SRC_STATE.pt OUT_DIR --set practice.human.enemy_coef=0.3 ...

Copies the state (weights, optimizer, EMAs, decision count) to OUT_DIR/state.pt with the config
keys changed (`--resume` only allows speed flags, so the learning settings change here, once).
"""
import argparse
import json
import pathlib

import torch

ap = argparse.ArgumentParser()
ap.add_argument("src")
ap.add_argument("out")
ap.add_argument("--set", action="append", default=[])
a = ap.parse_args()
s = torch.load(a.src, map_location="cpu", weights_only=False)
cfg = s["config"]
for kv in a.set:
    k, v = kv.split("=", 1)
    d = cfg
    *path, last = k.split(".")
    for p in path:
        d = d[p]
    print(f"{k}: {d.get(last)!r} -> {json.loads(v)!r}")
    d[last] = json.loads(v)
out = pathlib.Path(a.out).expanduser()
out.mkdir(parents=True, exist_ok=True)
torch.save(s, out / "state.pt")
print("decisions", s.get("decisions"), "->", out / "state.pt")
