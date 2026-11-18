"""RLN34: do act-3 boss practice starts play like real act-3 boss entries?

    STS2_FFI_LIB=.../NECRO1/lib/libsts2_ffi.so STS2_FFI_BUILD_ID=<its build> \\
        python python/rln34_act3.py CKPT [--weights ema0.999] [--starts 300]

Greedy play of the checkpoint (as `e2e_eval.py run`) on `--envs` A10 Necrobinder runs from seed
26,000,000 on; envs `0..slots` start BOSS_ACT3 practice snapshots (reseeded, `sts2_rl::practice`)
taken by the normal runs. Per side it counts entries into the act's first boss and how many
killed it (1 or 2 bosses) and won the run (both bosses), and how the practice episodes ended.
Prints one JSON line.

`--share DECISIONS` measures the cell shares instead: practice on `--slots` envs with the
trainer's schedule weights at DECISIONS (`train_e2e.Practice`, start 0) and `--max-uses`,
`--min-active`; prints the intended and the measured share of starts per cell over the second
half of the run.
"""

import argparse
import json
import pathlib
import sys
import time

import numpy as np
import torch

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import e2e_eval as V  # noqa: E402
from sts2 import E2E, Sts2RunEnvs  # noqa: E402
from sts2.e2e import encode as E  # noqa: E402
from sts2.env import RUN_ENDS  # noqa: E402
from sts2.ffi import CELL_NAMES  # noqa: E402

BOSS3 = CELL_NAMES.index("BOSS_ACT3")


def main(argv=None):
    ap = argparse.ArgumentParser()
    ap.add_argument("ckpt")
    ap.add_argument("--weights", default="ema0.999")
    ap.add_argument("--envs", type=int, default=1024)
    ap.add_argument("--slots", type=int, default=256)
    ap.add_argument("--starts", type=int, default=300, help="practice episodes to finish")
    ap.add_argument("--minutes", type=float, default=12.0)
    ap.add_argument("--threads", type=int, default=8)
    ap.add_argument("--share", type=int, default=None, help="schedule decisions (share mode)")
    ap.add_argument("--max-uses", type=int, default=4)
    ap.add_argument("--min-active", type=int, default=8)
    args = ap.parse_args(argv)
    dev = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    net = V.load_policy(args.ckpt, args.weights, dev)
    v2, full = bool(getattr(net, "obs_v2", False)), bool(getattr(net, "obs_full", False))
    events = bool(getattr(net, "obs_events", False))
    n = args.envs
    env = Sts2RunEnvs(n, seed_range=(26_000_000, 27_000_000), character=V.CHARACTER,
                      ascension=V.ASCENSION, combat="agent", threads=args.threads,
                      **(dict(obs_v2=True) if v2 else {}), **(dict(obs_full=True) if full else {}),
                      **(dict(obs_events=True) if events else {}),
                      **getattr(net, "env_opts", {}), **E2E)
    if args.share is not None:
        import train_e2e as T
        _, weights = T.Practice(start=0).schedule(args.share, n)
        env.set_practice(slots=args.slots, weights=weights, pool=32, max_uses=args.max_uses,
                         min_active=args.min_active, capture=True, seed=34)
    else:
        weights = [0] * 6
        weights[BOSS3] = 1
        env.set_practice(slots=args.slots, weights=weights, pool=64, max_uses=4, min_active=1,
                         capture=True, seed=34)
    half = None
    names = V.encounter_names()
    is_boss = np.array([s.endswith("_BOSS") for s in names] + [False] * 70000)
    bosses = np.zeros(n, np.int64)  # bosses killed this episode
    act3 = np.zeros(n, bool)  # a normal run entered an act-3 boss fight
    real = dict(entries=0, killed=0, won=0)
    prac = dict(episodes=0, killed=0, won=0, ends={})
    t0 = time.time()
    steps = 0
    try:
        while ((args.share is not None or prac["episodes"] < args.starts)
               and time.time() - t0 < args.minutes * 60):
            if half is None and time.time() - t0 > args.minutes * 30:
                half = env.practice_stats()["started"]
            obs_np = env.obs
            enc = obs_np[:, V.H_MODEL].astype(np.int64) | obs_np[:, V.H_MODEL + 1].astype(np.int64) << 8
            in_boss3 = (obs_np[:, V.H_ROOM] != 0) & (obs_np[:, V.H_ACT] == 2) & is_boss[enc]
            act3 |= in_boss3 & (env.cell == 0)
            obs = torch.from_numpy(obs_np).to(dev)
            masks = torch.from_numpy(env.masks.view(np.int64)).to(dev)
            with torch.no_grad():
                tok = E.encode(obs, masks, v2=v2, full=full, events=events,
                               discard=bool(getattr(net, "env_opts", {}).get("discard_potion")))
                with torch.autocast(dev.type, dtype=torch.bfloat16, enabled=dev.type == "cuda"):
                    logits, _, _ = net.policy(tok)
                action = logits.float().nan_to_num(nan=float("-inf")).argmax(-1)
            env.step(action.cpu().numpy().astype(np.uint16))
            steps += 1
            bosses += env.events[:, 1]
            done = env.done.astype(bool)
            for i in np.nonzero(done)[0]:
                cell = int(env.ended_cell[i])
                won = bool(env.won[i])
                if cell == BOSS3 + 1:
                    prac["episodes"] += 1
                    prac["killed"] += int(bosses[i] >= 1)
                    prac["won"] += int(won)
                    e = RUN_ENDS[env.end[i]]
                    prac["ends"][e] = prac["ends"].get(e, 0) + 1
                elif cell == 0 and act3[i]:
                    real["entries"] += 1
                    # A normal run that entered the act-3 boss: it killed it if the episode's
                    # bosses include an act-3 kill, i.e. 3 or more over the run (acts 1, 2, 3).
                    real["killed"] += int(bosses[i] >= 3)
                    real["won"] += int(won)
                bosses[i] = 0
                act3[i] = False
    finally:
        st = env.practice_stats()
        env.close()
    if args.share is not None:
        d = [b - a for a, b in zip(half, st["started"])]
        tot = max(sum(d), 1)
        print(json.dumps(dict(
            decisions=args.share, max_uses=args.max_uses, min_active=args.min_active,
            steps=steps, envs=n, slots=args.slots, starts=sum(d), to_run=sum(st["to_run"]),
            intended={c: round(w / sum(weights), 3) for c, w in zip(CELL_NAMES, weights)},
            measured={c: round(x / tot, 3) for c, x in zip(CELL_NAMES, d)},
            pool=dict(zip(CELL_NAMES, st["pool"])))))
        return
    for d in (real, prac):
        k, m = d["killed"], d.get("entries", d.get("episodes"))
        d["kill_rate"] = round(k / m, 4) if m else None
        d["kill_ci95"] = [round(x, 4) for x in V.wilson(k, m)] if m else None
    print(json.dumps(dict(ckpt=args.ckpt, weights=args.weights, steps=steps,
                          seconds=round(time.time() - t0, 1), real=real, practice=prac,
                          captured=st["captured"][BOSS3], started=st["started"][BOSS3])))


if __name__ == "__main__":
    main()
