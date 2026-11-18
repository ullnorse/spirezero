"""RLN23 item 2: how the policy handles reward screens (RLN20 suspect 5).

    python python/rln23_rewards.py CKPT --weights ema0.999 --n 500 --out F.json

Plays greedy DEV runs (seeds 24,000,000+, `e2e_eval.play_batch`'s loop) and, at every decision on a
rewards screen, reads the screen from the obs (`run_obs.rs`: REWARDS_AT, kind/value per reward) and
the action taken. Per screen it counts each reward as claimed (claim / pick a card / a card
alternative), skipped (`SkipReward`) or left (still listed when the policy chose `Proceed`).
A potion reward is claimable only with a free belt slot (`Run::push_reward_actions`), so a left
potion is split into "free slot" (an avoidable loss) and "belt full" (not claimable).
"""

import argparse
import json
import pathlib
import sys
import time
from collections import Counter

import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import e2e_eval as EV  # noqa: E402

# Run header and sections (`run_obs.rs`).
H_PHASE, H_GOLD, H_BELT_SLOTS = 3, 12, 15
BELT_AT, REWARDS_AT, REWARD_BYTES, REWARDS = 480, 1024, 21, 8
PHASE_REWARDS = 4
KINDS = {1: "gold", 2: "potion", 3: "relic", 4: "wax_relic", 5: "card", 6: "given_card"}

# Run action space (`run_action.rs`); the combat space has 140 indices.
CLAIM, PICK, ALT, SKIP, USE_POTION, PROCEED = 164, 172, 212, 228, 244, 354
DISCARD = 355  # RLN32: discard the potion in belt slot s, DISCARD + s
CARD_SLOTS, ALT_SLOTS, POTION_SLOTS = 5, 2, 6


def u16(row, at):
    return int(row[at]) | int(row[at + 1]) << 8


def screen(row):
    """`[(kind, value)]` of the rewards listed, in slot order (empty slots dropped, index kept)."""
    out = []
    for r in range(REWARDS):
        at = REWARDS_AT + r * REWARD_BYTES
        kind = int(row[at])
        out.append((KINDS.get(kind), u16(row, at + 2)) if kind else None)
    return out


def free_slots(row):
    cap = int(row[H_BELT_SLOTS])
    held = sum(1 for k in range(POTION_SLOTS) if u16(row, BELT_AT + 2 * k))
    return max(0, cap - held)


def play(net, seeds, device, threads):
    import torch

    from sts2 import E2E, HOLD, Sts2RunEnvs
    from sts2.e2e import encode as E

    n = len(seeds)
    v2 = bool(getattr(net, "obs_v2", False))
    env = Sts2RunEnvs(n, seeds=np.asarray(seeds, dtype=np.uint64), character=EV.CHARACTER,
                      ascension=EV.ASCENSION, combat="agent", threads=threads,
                      **(dict(obs_v2=True) if v2 else {}), **E2E)
    E.check_layout(env.layout, v2=v2)
    finished = np.zeros(n, dtype=bool)
    in_screen = np.zeros(n, dtype=bool)
    stats = [Counter() for _ in range(n)]  # per run
    floor_of = [[] for _ in range(n)]
    steps = 0
    try:
        while not finished.all():
            active = np.nonzero(~finished)[0]
            obs = torch.from_numpy(env.obs[active]).to(device)
            masks = torch.from_numpy(env.masks.view(np.int64)[active]).to(device)
            with torch.no_grad():
                tok = E.encode(obs, masks, v2=v2)
                with torch.autocast(device.type, dtype=torch.bfloat16,
                                    enabled=device.type == "cuda"):
                    logits, _, _ = net.policy(tok)
                action = logits.float().nan_to_num(nan=float("-inf")).argmax(-1).cpu().numpy()
            for j, i in enumerate(active):
                row, a = env.obs[i], int(action[j])
                if int(row[H_PHASE]) != PHASE_REWARDS:
                    in_screen[i] = False
                    continue
                s = screen(row)
                st = stats[i]
                if not in_screen[i]:  # first decision of a screen
                    in_screen[i] = True
                    st["screens"] += 1
                    for rw in s:
                        if rw:
                            st[f"shown_{rw[0]}"] += 1
                            if rw[0] == "gold":
                                st["gold_shown"] += rw[1]
                if CLAIM <= a < CLAIM + REWARDS:
                    rw = s[a - CLAIM]
                    st[f"claimed_{rw[0]}"] += 1
                    if rw[0] == "gold":
                        st["gold_claimed"] += rw[1]
                elif PICK <= a < ALT:
                    st["claimed_card"] += 1
                elif ALT <= a < SKIP:
                    st["claimed_card_alt"] += 1
                elif SKIP <= a < SKIP + REWARDS:
                    rw = s[a - SKIP]
                    st[f"skipped_{rw[0]}"] += 1
                    if rw[0] == "gold":
                        st["gold_skipped"] += rw[1]
                    if rw[0] == "potion":
                        st["potion_skipped_free" if free_slots(row) else "potion_skipped_full"] += 1
                elif a == PROCEED:
                    st["proceed"] += 1
                    free = free_slots(row)
                    left_potions = 0
                    for rw in s:
                        if not rw:
                            continue
                        st[f"left_{rw[0]}"] += 1
                        if rw[0] == "gold":
                            st["gold_left"] += rw[1]
                        if rw[0] == "potion":
                            left_potions += 1
                            st["potion_left_free" if free > left_potions - 1 else
                              "potion_left_full"] += 1
                    if any(s):
                        st["proceed_with_unclaimed"] += 1
                elif USE_POTION <= a < USE_POTION + POTION_SLOTS:
                    st["use_potion_on_screen"] += 1
                else:
                    raise SystemExit(f"unexpected action {a} on a rewards screen")
            step_actions = np.full(n, 0xFFFF, dtype=np.uint16)
            step_actions[:] = HOLD
            step_actions[active] = action
            env.step(step_actions)
            steps += 1
            for i in active[env.done[active].astype(bool)]:
                finished[i] = True
    finally:
        env.close()
    return stats


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("ckpt")
    p.add_argument("--weights", default="ema0.999")
    p.add_argument("--n", type=int, default=500)
    p.add_argument("--batch", type=int, default=250)
    p.add_argument("--threads", type=int, default=2)
    p.add_argument("--device", default="cuda")
    p.add_argument("--out", required=True)
    args = p.parse_args()
    import torch

    device = torch.device(args.device)
    net = EV.load_policy(args.ckpt, args.weights, device)
    first, _ = EV.panel_seeds("DEV-1000")
    seeds = list(range(first, first + args.n))
    per_run, t0 = [], time.time()
    for k in range(0, args.n, args.batch):
        per_run += play(net, seeds[k:k + args.batch], device, args.threads)
        print(f"{len(per_run)}/{args.n} runs  {time.time() - t0:.0f}s", flush=True)
    total = Counter()
    for st in per_run:
        total.update(st)
    runs = len(per_run)
    result = {"ckpt_sha256": EV.file_sha256(args.ckpt), "weights": args.weights, "runs": runs,
              "first_seed": first, "total": dict(sorted(total.items())),
              "per_run_mean": {k: v / runs for k, v in sorted(total.items())},
              "per_run": [dict(s) for s in per_run]}
    pathlib.Path(args.out).write_text(json.dumps(result))
    for k, v in sorted(total.items()):
        print(f"{k:28s} {v:8d}  per run {v / runs:7.2f}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
