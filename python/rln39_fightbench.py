"""RLN39 fight benchmark: greedy win rate per boss from a fixed panel of boss-entry states.

    python python/rln39_fightbench.py build H.gz... --out PANEL [--kaiser 200 --insatiable 200 --demon 100 --act3 150]
    python python/rln39_fightbench.py run CKPT [--panel PANEL] [--weights ema0.999] [--out F]
    python python/rln39_fightbench.py compare A.json B.json            # paired, per boss (A minus B)

**Panel** (`build`): the first N boss-entry records per boss, in harvest order (`rln36_boss.py
harvest`: DEV and SMOKE seeds, never TEST), as run seed + action prefix (base64 uint16) + the v1
digest (`rln19_act2.sha`) of the obs row at the fight's first decision + entry HP. Act-3 bosses
take up to `--act3` states each; every entry is real (none are practice starts).

**Run**: replays each prefix in a fresh env (HOLD keeps envs in step), refuses a prefix whose digest
differs (engine change: rebuild the panel), then plays every fight greedily with the net until it
ends. Greedy play is deterministic. Reports per boss: wins, win %, Wilson 95% CI, and the mean entry
HP %. One env per fight; <= 2 CPU threads and a 3 GB GPU cap (`rln36_boss.device_and_net`), run
under `flock ~/spirebot/target/gpu.lock`.

**Compare**: paired on fight id; difference in win rate with Newcombe's 95% CI
(`e2e_eval.newcombe_paired`), per boss and over all fights.
"""

import argparse
import collections
import json
import pathlib
import sys
import time

import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import e2e_eval as EV  # noqa: E402
import rln19_act2 as R  # noqa: E402
import rln36_boss as B  # noqa: E402

DEFAULT_PANEL = pathlib.Path(__file__).resolve().parent / "rln39/panel-v1.jsonl.gz"
ACT3 = ("AEONGLASS_BOSS", "QUEEN_BOSS", "TEST_SUBJECT_BOSS", "DOORMAKER_BOSS")
STEP_CAP = 4000
CHUNK = 512  # rows per net call (the 3 GB GPU cap)


def fight_id(f):
    return f"{f['seed']}-{f['floor']}"


def select(fights, limits):
    """First `limits[boss]` fights per boss, in the order given (a boss absent from `limits` is
    skipped). Duplicates (same seed and floor) are dropped."""
    out, seen, count = [], set(), collections.Counter()
    for f in fights:
        b = f["boss"]
        if b not in limits or count[b] >= limits[b] or fight_id(f) in seen:
            continue
        seen.add(fight_id(f))
        count[b] += 1
        out.append({"id": fight_id(f), "seed": f["seed"], "boss": b, "act": f["act"],
                    "floor": f["floor"], "prefix": f["prefix"], "sha_v1": f["sha_v1"],
                    "hp": f["entry"]["hp"], "max_hp": f["entry"]["max_hp"]})
    return out


def cmd_build(args):
    fights = []
    for p in args.harvests:
        meta, rows = R.read_jsonl(p)
        fights += rows
    limits = {"KAISER_CRAB_BOSS": args.kaiser, "THE_INSATIABLE_BOSS": args.insatiable,
              "KNOWLEDGE_DEMON_BOSS": args.demon, **{b: args.act3 for b in ACT3}}
    panel = select(fights, limits)
    R.write_jsonl_gz(args.out, panel, {"kind": "rln39-fightbench-panel", "harvests": args.harvests,
                                       "limits": limits, "lib_build": "e6dd5bfdabd8089f",
                                       "commit": EV.code_commit()})
    c = collections.Counter(f["boss"] for f in panel)
    print(f"{len(panel)} fights: {dict(c)}")
    short = [b for b, n in limits.items() if c[b] < min(n, 100) and b in R.BOSSES + ACT3[:3]]
    if sum(c[b] for b in ACT3) < 100 or short:
        print("WARNING: fewer fights than asked", short)
    return 0


def play(net, dev, panel):
    """Greedy outcome of every panel fight: [{id, boss, win, steps, hp}]."""
    from sts2 import HOLD

    env = B.replay(net, panel, 1)
    n = len(panel)
    live = np.ones(n, dtype=bool)
    res = [None] * n
    steps = np.zeros(n, dtype=np.int64)
    try:
        for _ in range(STEP_CAP):
            idx = np.nonzero(live)[0]
            if len(idx) == 0:
                break
            a = np.concatenate([B.logits_of(net, env, idx[k:k + CHUNK], dev).argmax(-1).cpu().numpy()
                                for k in range(0, len(idx), CHUNK)])
            acts = np.full(n, HOLD, dtype=np.uint16)
            acts[idx] = a
            env.step(acts)
            steps[idx] += 1
            for i in idx:
                o = B.outcome(env, i)
                if o is not None:
                    live[i] = False
                    res[i] = {"id": panel[i]["id"], "boss": panel[i]["boss"],
                              "win": int(o[0] == "won"), "end": o[0], "steps": int(steps[i]),
                              "hp_left": int(o[1])}
        for i in np.nonzero(live)[0]:
            res[i] = {"id": panel[i]["id"], "boss": panel[i]["boss"], "win": 0, "end": "capped",
                      "steps": int(steps[i]), "hp_left": 0}
    finally:
        env.close()
    return res


def summarize(panel, res):
    by = collections.OrderedDict()
    hp = {f["id"]: f["hp"] / max(1, f["max_hp"]) for f in panel}
    for r in res:
        by.setdefault(r["boss"], []).append(r)
    rows = []
    for b, rs in list(by.items()) + [("ALL", res)]:
        k, n = sum(r["win"] for r in rs), len(rs)
        lo, hi = EV.wilson(k, n)
        rows.append({"boss": b, "wins": k, "n": n, "win": k / n, "lo": lo, "hi": hi,
                     "entry_hp": float(np.mean([hp[r["id"]] for r in rs]))})
    return rows


def show(rows):
    print("| boss | wins | n | win % | Wilson 95% | entry HP % |\n|---|---:|---:|---:|---|---:|")
    for r in rows:
        print(f"| {r['boss']} | {r['wins']} | {r['n']} | {100 * r['win']:.1f} "
              f"| {100 * r['lo']:.1f}-{100 * r['hi']:.1f} | {100 * r['entry_hp']:.0f} |")


def cmd_run(args):
    t0 = time.time()
    _, panel = R.read_jsonl(args.panel)
    dev, net = B.device_and_net(args.ckpt, args.weights)
    res = play(net, dev, panel)
    rows = summarize(panel, res)
    show(rows)
    secs = time.time() - t0
    print(f"{len(panel)} fights in {secs:.0f}s")
    if args.out:
        pathlib.Path(args.out).write_text(json.dumps(
            {"ckpt": str(args.ckpt), "ckpt_sha256": EV.file_sha256(args.ckpt),
             "weights": args.weights, "panel": str(args.panel), "seconds": secs,
             "summary": rows, "fights": res}))
    return 0


def paired(a, b):
    """Per boss (and ALL): wins of A and B on the shared fights and Newcombe's CI of A - B."""
    wa = {r["id"]: r for r in a["fights"]}
    wb = {r["id"]: r for r in b["fights"]}
    ids = [i for i in wa if i in wb]
    out = []
    bosses = list(dict.fromkeys(wa[i]["boss"] for i in ids)) + ["ALL"]
    for boss in bosses:
        sel = [i for i in ids if boss == "ALL" or wa[i]["boss"] == boss]
        both = sum(wa[i]["win"] and wb[i]["win"] for i in sel)
        only_a = sum(wa[i]["win"] and not wb[i]["win"] for i in sel)
        only_b = sum(wb[i]["win"] and not wa[i]["win"] for i in sel)
        neither = len(sel) - both - only_a - only_b
        _, lo, hi = EV.newcombe_paired(both, only_a, only_b, neither)
        n = len(sel)
        out.append({"boss": boss, "n": n, "a": (both + only_a), "b": (both + only_b),
                    "both": both, "only_a": only_a, "only_b": only_b, "diff": (only_a - only_b) / n,
                    "lo": lo, "hi": hi})
    return out


def cmd_compare(args):
    a, b = (json.loads(pathlib.Path(p).read_text()) for p in (args.a, args.b))
    print("| boss | n | A wins | B wins | A only | B only | diff pp | 95% |\n|---|---:|---:|---:|---:|---:|---:|---|")
    for r in paired(a, b):
        print(f"| {r['boss']} | {r['n']} | {r['a']} | {r['b']} | {r['only_a']} | {r['only_b']} "
              f"| {100 * r['diff']:+.1f} | {100 * r['lo']:+.1f} to {100 * r['hi']:+.1f} |")
    return 0


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    b = sub.add_parser("build")
    b.add_argument("harvests", nargs="+")
    b.add_argument("--kaiser", type=int, default=200)
    b.add_argument("--insatiable", type=int, default=200)
    b.add_argument("--demon", type=int, default=100)
    b.add_argument("--act3", type=int, default=150, help="per act-3 boss")
    b.add_argument("--out", default=str(DEFAULT_PANEL))
    r = sub.add_parser("run")
    r.add_argument("ckpt")
    r.add_argument("--panel", default=str(DEFAULT_PANEL))
    r.add_argument("--weights", default="ema0.999")
    r.add_argument("--out")
    c = sub.add_parser("compare")
    c.add_argument("a")
    c.add_argument("b")
    args = p.parse_args()
    return {"build": cmd_build, "run": cmd_run, "compare": cmd_compare}[args.cmd](args)


if __name__ == "__main__":
    raise SystemExit(main())
