"""CB1 diagnostic: how winnable are the benchmark's fights for this net's style of play?

    python tools/cb1/winnable.py CKPT --room boss --n 16 --temp 1.0 --out F.npz

Each start of the room is set up as in the benchmark (sequential gym, seed 7,000,000 + k: rollout 0
of `naz3_combat.py run`). At its first decision the fight is copied `n` times exactly (same draw
order and streams: a clairvoyant world) and `n` times reseeded (`Sts2RunEnvs.copy(reseed)`: honest
worlds); every copy is played to the fight's end by the net, sampling at `temp`. Per start: wins
of the exact copies and of the reseeded copies. "Any exact copy won" is a lower bound on the start
being winnable with perfect information; the reseeded mean is the sampled policy's win rate.
"""

import argparse
import json
import os
import pathlib
import sys

import numpy as np

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "python"))


def main():
    p = argparse.ArgumentParser()
    p.add_argument("ckpt")
    p.add_argument("--weights", default="raw")
    p.add_argument("--room", default="boss")
    p.add_argument("--n", type=int, default=16)
    p.add_argument("--temp", type=float, default=1.0)
    p.add_argument("--threads", type=int, default=6)
    p.add_argument("--out", required=True)
    a = p.parse_args()
    import torch

    import naz3_combat as N
    import rln36_boss as B
    from sts2 import HOLD
    from sts2.env import RUN_ENDS

    net, dev = N.net_and_dev(a.ckpt, a.weights)
    recs = [r for r in N.read_jsonl(N.DATA / "bench.jsonl") if N.room_of(r) == a.room]
    F, n = len(recs), a.n
    tmp = ROOT / f"target/cb1/win-gym-{os.getpid()}.jsonl"
    tmp.write_text("".join(N.engine_line(r) + "\n" for r in recs))
    B.THREADS = a.threads
    total = F + 2 * F * n
    env = B.make_env(net, total, seed_range=(0, 1), cycle=True)
    gen = torch.Generator(device=dev)
    gen.manual_seed(0)
    rng = np.random.default_rng(0)
    try:
        assert env.load_gym(tmp) == F
        env.set_gym(first=0, slots=F, sequential=True, seed=7_000_000)
        env.reset_all()
        for i in range(F):
            got = json.loads(env.start_record(i) or "{}")
            assert got.get("encounter") == recs[i]["encounter"], (i, got.get("encounter"))
        src = np.repeat(np.arange(F), n)
        exact = F + np.arange(F * n)
        resd = F + F * n + np.arange(F * n)
        env.copy(src, exact)
        env.copy(src, resd, reseed=rng.integers(0, 2**63, size=F * n, dtype=np.uint64))
        running = np.concatenate([exact, resd])
        won = np.zeros(total, dtype=bool)
        steps = 0
        while len(running) and steps < N.ROLLOUT_CAP:
            done = env.done[running].astype(bool)
            fin = done | (env.obs[running, N.H_PHASE] != N.PHASE_COMBAT)
            if fin.any():
                d = running[fin]
                ends = np.array([RUN_ENDS[e] for e in env.end[d]])
                won[d] = ~env.done[d].astype(bool) | np.isin(ends, ("gym_end", "victory"))
            running = running[~fin]
            if not len(running):
                break
            lg = N.logits(net, env, running, dev)
            if a.temp > 0:
                act = torch.multinomial(torch.softmax(lg / a.temp, -1), 1, generator=gen)
                act = act.squeeze(1).cpu().numpy()
            else:
                act = lg.argmax(-1).cpu().numpy()
            acts = np.full(total, HOLD, dtype=np.uint16)
            acts[running] = act
            env.step(acts)
            steps += 1
            if steps % 50 == 0:
                print(f"step {steps}: {len(running)} running", flush=True)
    finally:
        env.close()
        tmp.unlink(missing_ok=True)
    we = won[exact].reshape(F, n)
    wr = won[resd].reshape(F, n)
    act = np.array([r["act"] for r in recs])
    np.savez(a.out, ids=np.array([r["id"] for r in recs]), act=act,
             enc=np.array([r["encounter"] for r in recs]), exact=we, reseeded=wr)
    for k in range(3):
        s = act == k
        print(f"act {k + 1}: {s.sum()} starts; exact worlds: any win {100 * we[s].any(1).mean():.1f}%, "
              f"mean win {100 * we[s].mean():.1f}%; reseeded: any win "
              f"{100 * wr[s].any(1).mean():.1f}%, mean win {100 * wr[s].mean():.1f}%")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
