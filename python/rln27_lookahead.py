"""RLN27: does a net-guided lookahead beat the net's greedy play in act-2 boss fights?

    python python/rln27_lookahead.py run CKPT --bench BENCH --k 4 --n 8 --out F   # lookahead
    python python/rln27_lookahead.py run CKPT --bench BENCH --k 0 --out F         # greedy
    python python/rln27_lookahead.py report GREEDY LOOKAHEAD...

At each agent decision of the fight the net's top-`k` actions (by policy) are tried: the fight state is
copied (`Sts2RunEnvs.copy`) `n` times per action, each copy determinized with its own seed (RLN28:
draw pile reshuffled, the fight's random streams reseeded; `--leaky` keeps the real ones), the action
is applied and the fight is played to its end with the net sampling at temperature 1. The action with
the most wins (then the most HP left, deaths counting 0) is played; ties go to the higher policy rank.
`--k 0` plays the net greedily with the same machinery (the paired baseline). Fights replay their
prefix as `rln19_act2.py bench` does (sha-checked). Envs per fight: 1 + k*n, on <= 4 threads.
"""

import argparse
import math
import pathlib
import sys
import time

import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import e2e_eval as EV  # noqa: E402
import rln19_act2 as R  # noqa: E402

MAX_ROLLOUT_STEPS = 4000


def logits_of(net, env, idx, device):
    """Masked policy logits (rows of `idx`), float32 numpy, -inf where illegal."""
    import torch

    from sts2.e2e import encode as E

    v2 = bool(getattr(net, "obs_v2", False))
    obs = torch.from_numpy(env.obs[idx]).to(device)
    masks = torch.from_numpy(env.masks.view(np.int64)[idx]).to(device)
    with torch.no_grad():
        tok = E.encode(obs, masks, v2=v2)
        with torch.autocast(device.type, dtype=torch.bfloat16, enabled=device.type == "cuda"):
            logits, _, _ = net.policy(tok)
        logits = logits.float().nan_to_num(nan=float("-inf"))
        legal = E.mask_bits(masks)
        logits = logits.masked_fill(~legal, float("-inf"))
    return logits


def replay(entries, per):
    """Env batch of len(entries) * per slots; slot f*per replays entry f's prefix (sha-checked)."""
    from sts2 import E2E, HOLD, Sts2RunEnvs

    n = len(entries) * per
    seeds = np.asarray([e["seed"] for e in entries for _ in range(per)], dtype=np.uint64)
    env = Sts2RunEnvs(n, seeds=seeds, character=EV.CHARACTER, ascension=EV.ASCENSION,
                      combat="agent", threads=R.THREADS, obs_v2=True, **E2E)
    if int(env.layout.v2_at) != R.V1_BYTES:
        raise SystemExit("the obs v1 layout moved")
    prefixes = [EV.unpack(e["prefix"]) for e in entries]
    acts = np.full((max(len(p) for p in prefixes), n), HOLD, dtype=np.uint16)
    for f, p in enumerate(prefixes):
        acts[: len(p), f * per] = p
    for t in range(len(acts)):
        env.step(acts[t])
        if env.done.any():
            raise SystemExit("a prefix ended its run")
    bad = [entries[f]["id"] for f in range(len(entries))
           if R.sha(env.obs[f * per]) != entries[f]["sha_v1"]]
    if bad:
        raise SystemExit(f"{len(bad)} prefixes reach a different state (e.g. {bad[:3]})")
    return env


def outcome(env, i):
    """(finished, won, hp left) of slot `i` after a step."""
    row = env.obs[i]
    if env.events[i, 1] > 0:
        return True, 1, max(0, R.i16(row, R.H_HP))
    if env.done[i] or int(row[R.H_PHASE]) != R.PHASE_COMBAT:
        return True, 0, 0
    return False, 0, 0


def play_chunk(net, device, entries, k, n, gen, temp, seeds, leaky=False):
    """Plays `entries` to their end. `seeds` (a numpy Generator) draws one determinization seed
    per rollout copy; the real fight keeps its own streams."""
    import torch

    from sts2 import HOLD

    m = k * n
    per = 1 + m
    nf = len(entries)
    env = replay(entries, per)
    main = np.arange(nf) * per
    N = env.n
    live = np.ones(nf, dtype=bool)
    res = [{"id": e["id"], "boss": e["boss"], "won": 0, "hp": 0, "decisions": 0} for e in entries]
    t_search = 0
    while live.any():
        fs = np.nonzero(live)[0]
        lg = logits_of(net, env, main[fs], device)
        top = torch.topk(lg, min(max(k, 1), lg.shape[1]), dim=1)
        vals, ids = top.values.cpu().numpy(), top.indices.cpu().numpy()
        choice = {}
        pending = []
        for row, f in enumerate(fs):
            cand = [int(a) for a, v in zip(ids[row], vals[row]) if np.isfinite(v)]
            if k == 0 or len(cand) == 1:
                choice[f] = cand[0]
            else:
                pending.append((f, cand))
        if pending:
            src, dst, act, owner = [], [], [], []
            for f, cand in pending:
                for j, a in enumerate(cand):
                    for r in range(n):
                        d = main[f] + 1 + j * n + r
                        src.append(main[f]); dst.append(d); act.append(a); owner.append((f, j))
            src, dst = np.asarray(src), np.asarray(dst)
            reseed = None if leaky else seeds.integers(0, 2**63, size=len(src), dtype=np.uint64)
            env.copy(src, dst, reseed=reseed)
            acts = np.full(N, HOLD, dtype=np.uint16)
            acts[dst] = act
            env.step(acts)
            wins = {}
            hps = {}
            running = []
            for d, (f, j) in zip(dst, owner):
                fin, w, hp = outcome(env, d)
                if fin:
                    wins[d], hps[d] = w, hp
                else:
                    running.append(d)
            steps = 0
            while running and steps < MAX_ROLLOUT_STEPS:
                idx = np.asarray(running)
                lgr = logits_of(net, env, idx, device) / temp
                a = torch.multinomial(torch.softmax(lgr, -1), 1, generator=gen).squeeze(1).cpu().numpy()
                acts = np.full(N, HOLD, dtype=np.uint16)
                acts[idx] = a
                env.step(acts)
                nxt = []
                for d in running:
                    fin, w, hp = outcome(env, d)
                    if fin:
                        wins[d], hps[d] = w, hp
                    else:
                        nxt.append(d)
                running = nxt
                steps += 1
            for d in running:  # step-cap: counts as a loss
                wins[d], hps[d] = 0, 0
            score = {}
            for d, (f, j) in zip(dst, owner):
                s = score.setdefault((f, j), [0, 0])
                s[0] += wins[d]
                s[1] += hps[d]
            for f, cand in pending:
                best = max(range(len(cand)), key=lambda j: (score[(f, j)][0], score[(f, j)][1], -j))
                choice[f] = cand[best]
        acts = np.full(N, HOLD, dtype=np.uint16)
        for f, a in choice.items():
            acts[main[f]] = a
            res[f]["decisions"] += 1
        env.step(acts)
        for f in fs:
            fin, w, hp = outcome(env, main[f])
            if fin:
                res[f]["won"], res[f]["hp"] = w, hp
                live[f] = False
    env.close()
    return res


def cmd_run(args):
    import torch

    meta, entries = R.read_jsonl(args.bench)
    net = R.load_net(args.ckpt, args.weights)
    device = torch.device(args.device)
    net = net.to(device)
    gen = torch.Generator(device=device)
    gen.manual_seed(args.seed)
    seeds = np.random.default_rng(args.seed)
    # Interleave the bosses so a deadline stops on a balanced subset.
    by = {b: [e for e in entries if e["boss"] == b] for b in R.BOSSES}
    if args.boss:
        by = {args.boss: by[args.boss]}
    if args.limit:
        by = {b: v[: args.limit] for b, v in by.items()}
    chunks = []
    for i in range(0, max(len(v) for v in by.values()), args.chunk):
        for b, v in by.items():
            if v[i:i + args.chunk]:
                chunks.append(v[i:i + args.chunk])
    t0 = time.time()
    out, decisions = [], 0
    for ci, chunk in enumerate(chunks):
        if time.time() - t0 > args.deadline:
            print(f"deadline: stopping after {ci}/{len(chunks)} chunks", flush=True)
            break
        t1 = time.time()
        rs = play_chunk(net, device, chunk, args.k, args.n, gen, args.temp, seeds, args.leaky)
        out += rs
        decisions += sum(r["decisions"] for r in rs)
        print(f"chunk {ci + 1}/{len(chunks)} {chunk[0]['boss']} {len(chunk)} fights "
              f"{time.time() - t1:.0f}s  wins {sum(r['won'] for r in rs)}/{len(rs)}  "
              f"total {len(out)} fights {time.time() - t0:.0f}s", flush=True)
        R.write_jsonl_gz(args.out, out, {
            "kind": "rln27-lookahead", "k": args.k, "n": args.n, "temp": args.temp,
            "reseed": not args.leaky,
            "seed": args.seed, "weights": args.weights, "wall_s": time.time() - t0,
            "decisions": decisions, "bench_sha256": EV.file_sha256(args.bench),
            "ckpt_sha256": EV.file_sha256(args.ckpt), "commit": EV.code_commit()})
    return 0


def cmd_report(args):
    _, g = R.read_jsonl(args.greedy)
    gid = {r["id"]: r for r in g}
    for path in args.lookahead:
        meta, rs = R.read_jsonl(path)
        print(f"\n## {path}: k={meta['k']} n={meta['n']} reseed={meta.get('reseed', False)}  "
              f"wall {meta['wall_s']:.0f}s  {meta['decisions']} decisions ({meta['decisions'] / meta['wall_s']:.1f}/s)\n")
        print("| boss | fights | greedy wins (95%) | lookahead wins (95%) | diff pp (95%) | both | greedy-only"
              " | search-only | neither | HP left greedy | HP left search |")
        print("|---|---:|---|---|---|---:|---:|---:|---:|---:|---:|")
        for b in R.BOSSES + ("ALL",):
            sel = [r for r in rs if (b == "ALL" or r["boss"] == b) and r["id"] in gid]
            if not sel:
                continue
            gs = [gid[r["id"]] for r in sel]
            n = len(sel)
            both = sum(1 for r, q in zip(sel, gs) if r["won"] and q["won"])
            gon = sum(1 for r, q in zip(sel, gs) if not r["won"] and q["won"])
            son = sum(1 for r, q in zip(sel, gs) if r["won"] and not q["won"])
            nei = n - both - gon - son
            kg, ks = both + gon, both + son
            lg, hg = EV.wilson(kg, n)
            ls, hs = EV.wilson(ks, n)
            _, lo, hi = EV.newcombe_paired(both, son, gon, nei)  # search minus greedy
            diff = f"{100 * (ks - kg) / n:+.1f} ({100 * lo:+.1f} to {100 * hi:+.1f})"
            print(f"| {b} | {n} | {kg} ({100 * kg / n:.1f}%, {100 * lg:.1f}-{100 * hg:.1f}) | "
                  f"{ks} ({100 * ks / n:.1f}%, {100 * ls:.1f}-{100 * hs:.1f}) | {diff} | {both} | "
                  f"{gon} | {son} | {nei} | {np.mean([q['hp'] for q in gs]):.1f} | "
                  f"{np.mean([r['hp'] for r in sel]):.1f} |")
    return 0


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("run")
    r.add_argument("ckpt")
    r.add_argument("--bench", required=True)
    r.add_argument("--weights", default="ema0.999")
    r.add_argument("--k", type=int, default=4, help="0: greedy")
    r.add_argument("--n", type=int, default=8)
    r.add_argument("--temp", type=float, default=1.0)
    r.add_argument("--leaky", action="store_true",
                   help="RLN27's copies: rollouts keep the real draw order and streams")
    r.add_argument("--seed", type=int, default=0)
    r.add_argument("--chunk", type=int, default=8, help="fights per env batch")
    r.add_argument("--boss")
    r.add_argument("--limit", type=int, help="fights per boss")
    r.add_argument("--deadline", type=float, default=1e9, help="seconds; no new chunk after it")
    r.add_argument("--device", default="cuda")
    r.add_argument("--out", required=True)
    q = sub.add_parser("report")
    q.add_argument("greedy")
    q.add_argument("lookahead", nargs="+")
    args = p.parse_args()
    return {"run": cmd_run, "report": cmd_report}[args.cmd](args)


if __name__ == "__main__":
    raise SystemExit(main())
