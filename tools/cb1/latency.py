"""CB1: inference cost of one decision (batch 1: encode + forward + mask + argmax).

    python tools/cb1/latency.py CKPT [--n 300] [--device cuda|cpu]

Plays benchmark fights one env at a time and times each decision's net call (median, p90, ms),
after 30 warm-up calls. The env step is not counted.
"""

import argparse
import json
import pathlib
import sys
import time

import numpy as np

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "python"))


def main():
    p = argparse.ArgumentParser()
    p.add_argument("ckpt")
    p.add_argument("--n", type=int, default=300)
    p.add_argument("--device", default="cuda")
    a = p.parse_args()
    import torch

    import e2e_eval as EV
    import naz3_combat as N
    import rln36_boss as B
    from sts2 import HOLD

    dev = torch.device(a.device)
    torch.set_num_threads(1)
    net = EV.load_policy(a.ckpt, "ema0.999", dev)
    recs = N.read_jsonl(N.DATA / "bench.jsonl")[:40]
    tmp = ROOT / "target/cb1/latency-gym.jsonl"
    tmp.write_text("".join(N.engine_line(r) + "\n" for r in recs))
    B.THREADS = 1
    env = B.make_env(net, 1, seed_range=(0, 1), cycle=True)
    times = []
    try:
        env.load_gym(tmp)
        env.set_gym(first=0, slots=1, sequential=True, seed=1)
        env.reset_all()
        idx = np.array([0])
        while len(times) < a.n + 30:
            if dev.type == "cuda":
                torch.cuda.synchronize()
            t0 = time.perf_counter()
            lg = N.logits(net, env, idx, dev)
            act = int(lg.argmax(-1).item())
            times.append(time.perf_counter() - t0)
            acts = np.full(1, HOLD, dtype=np.uint16)
            acts[0] = act
            env.step(acts)
    finally:
        env.close()
        tmp.unlink(missing_ok=True)
    t = np.array(times[30:]) * 1000
    print(json.dumps({"device": a.device, "n": len(t), "median_ms": round(float(np.median(t)), 2),
                      "p90_ms": round(float(np.percentile(t, 90)), 2)}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
