"""RLN36: why NECRO3 dies to bosses (`docs/research/rln36-boss-wall.md`).

    . target/rln36/env.sh   # the lane's frozen lib (STS2_FFI_LIB, STS2_FFI_BUILD_ID), 2 threads
    python python/rln36_boss.py harvest CKPT --first 24000000 --count 2000 --out F   # greedy runs
    python python/rln36_boss.py act3 CKPT --fights 60 --out F       # act-3 boss practice starts
    python python/rln36_boss.py cf CKPT --harvest F --per 30 --out F  # act-2 boss counterfactuals
    python python/rln36_boss.py report --harvest F --act3 F --cf F

**harvest** plays greedy runs exactly as `e2e_eval.py run` (same flags, `ema0.999`) and writes, per
run, its action hash (checked against an eval file with `--check`) and, for every boss fight, a
record: run seed, boss, act, floor, the action prefix to the fight's first decision and its v1 sha,
the entry state (HP, deck and card mix, relics, potions, gold) and one row per agent decision
(`ROW_FIELDS`: turn, HP, block, energy, visible incoming damage, Osty, enemy HP, belt size, the
action kind and its card or potion), the final row and how the fight ended.

**act3** traces act-3 boss fights from practice starts (`rln34_act3.py`'s setup: envs `0..slots`
start reseeded BOSS_ACT3 snapshots taken by the other envs' normal runs). At a practice start's
first decision the state is also copied (`Sts2RunEnvs.copy`, same streams) into spare envs that
play the same fight with all potions forced first (`potion`) and `--samples` sampled plays.

**cf** replays harvested act-2 boss prefixes (sha-checked) and plays each fight from the same
state as: `greedy` (must reproduce the harvest), `potion` (greedy, but every legal potion is
drunk as soon as it can be, the net's best target), `--samples` sampled plays at temperature 1
(same streams: what the net's own moves can reach), and an honest lookahead (`--k` top actions x
`--n` reseeded copies, each played out greedily; the action with the most wins, then HP, is
played).

GPU: per-process cap of 3 GB (`torch.cuda.set_per_process_memory_fraction`); run under
`flock ~/spirebot/target/gpu.lock`.
"""

import argparse
import collections
import json
import os
import pathlib
import sys
import time

import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import e2e_eval as EV  # noqa: E402
import rln19_act2 as R  # noqa: E402

THREADS = int(os.environ.get("STS2_THREADS", "2"))
RELICS_AT = 352
POTIONS = None
CARDS = None
RELICS = None


def vocab():
    global POTIONS, CARDS, RELICS
    if CARDS is None:
        CARDS = R._vocab("cards")
        POTIONS = R._vocab("potions")
        RELICS = R._vocab("relics")


def device_and_net(ckpt, weights):
    import torch

    torch.set_num_threads(THREADS)
    dev = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    if dev.type == "cuda":
        total = torch.cuda.get_device_properties(0).total_memory
        torch.cuda.set_per_process_memory_fraction(min(1.0, 3 * 2**30 / total))
    net = EV.load_policy(ckpt, weights, dev)
    return dev, net


def flags(net):
    return dict(v2=bool(getattr(net, "obs_v2", False)), full=bool(getattr(net, "obs_full", False)),
                events=bool(getattr(net, "obs_events", False)),
                discard=bool(getattr(net, "env_opts", {}).get("discard_potion")))


def make_env(net, n, **kw):
    from sts2 import E2E, Sts2RunEnvs
    from sts2.e2e import encode as E

    f = flags(net)
    env = Sts2RunEnvs(n, character=EV.CHARACTER, ascension=EV.ASCENSION, combat="agent",
                      threads=THREADS, **(dict(obs_v2=True) if f["v2"] else {}),
                      **(dict(obs_full=True) if f["full"] else {}),
                      **(dict(obs_events=True) if f["events"] else {}),
                      **getattr(net, "env_opts", {}), **E2E, **kw)
    E.check_layout(env.layout, v2=f["v2"], full=f["full"], events=f["events"])
    return env


def logits_of(net, env, idx, dev):
    """Masked float32 logits of rows `idx` (torch, on `dev`), -inf where illegal."""
    import torch

    from sts2.e2e import encode as E

    f = flags(net)
    obs = torch.from_numpy(env.obs[idx]).to(dev)
    masks = torch.from_numpy(env.masks.view(np.int64)[idx]).to(dev)
    with torch.no_grad():
        tok = E.encode(obs, masks, **f)
        with torch.autocast(dev.type, dtype=torch.bfloat16, enabled=dev.type == "cuda"):
            logits, _, _ = net.policy(tok)
        logits = logits.float().nan_to_num(nan=float("-inf"))
        legal = E.mask_bits(masks, discard=f["discard"])[:, : logits.shape[1]]
        return logits.masked_fill(~legal, float("-inf"))


_TABLE = None


def table():
    global _TABLE
    if _TABLE is None:
        from sts2 import action_table

        _TABLE = action_table()
    return _TABLE


KIND_HAND, KIND_POTION, KIND_END = 0, 1, 2
SMITH, REMOVE = 237, 349  # rest_option slot 1 (Heal, Smith, ...), shop_remove
SELECT0, SELECT_AT = 250, 1192  # select_option slot 0; `run_obs::SELECT_AT`


def removal_target(row, legal):
    """The deck-selection slot of a curse, else a starter Strike, else a starter Defend
    (unupgraded first), among legal options."""
    vocab()
    types = R.card_types()
    best = None
    for k in range(80):
        cid = R.u16(row, SELECT_AT + 8 + 5 * k)
        if not cid or not legal[SELECT0 + k]:
            continue
        name, up = CARDS[cid], int(row[SELECT_AT + 8 + 5 * k + 2])
        rank = (0 if types[cid] == "Curse" and name != "ASCENDERS_BANE" else
                1 if name == "STRIKE_NECROBINDER" else 2 if name == "DEFEND_NECROBINDER" else None)
        if rank is not None and (best is None or (rank, up) < best[0]):
            best = ((rank, up), k)
    return None if best is None else best[1]
overrides = collections.Counter()


def label(row, a):
    """The action's kind and its card or potion name (combat actions), else the kind."""
    vocab()
    k, slot = int(table()[a, 0]), int(table()[a, 1])
    if k == KIND_HAND:
        cid = R.played_card(row, slot)
        return "card", CARDS[cid] if cid < len(CARDS) else f"#{cid}"
    if k == KIND_POTION:
        return "potion", POTIONS[R.u16(row, R.BELT_AT + 2 * slot)]
    if k == KIND_END:
        return "end", ""
    from sts2.env import ENTITY_KINDS

    return ENTITY_KINDS[k], ""


def entry_of(row):
    vocab()
    e = R.entry_state(row)
    e["relic_names"] = [RELICS[R.u16(row, RELICS_AT + 4 * k)] for k in range(e["relics"])
                        if R.u16(row, RELICS_AT + 4 * k)]
    e["potion_names"] = [POTIONS[R.u16(row, R.BELT_AT + 2 * k)] for k in range(R.BELT_SLOTS)
                         if R.u16(row, R.BELT_AT + 2 * k)]
    return e


ROW_FIELDS = R.ROW_FIELDS[:-1] + ("kind", "what")


def fight_row(row, a):
    return R.combat_row(row) + list(label(row, a))


def fight_end(env, i):
    """None while the fight runs, else 'won', a run end ('died', ...) or 'left'."""
    from sts2.env import RUN_ENDS

    if env.events[i, 1] > 0:
        return "won"
    if env.done[i]:
        return RUN_ENDS[env.end[i]]
    if int(env.obs[i][R.H_PHASE]) != R.PHASE_COMBAT:
        return "left"
    return None


def final_row(env, i):
    return R.combat_row(env.final_obs[i] if env.done[i] else env.obs[i])


def write(path, rows, meta):
    R.write_jsonl_gz(path, rows, meta)


# ---- harvest ----


def harvest_batch(net, dev, seeds, smith=None, remove=False, target=False, unstick=False):
    from sts2 import E2E, HOLD

    n = len(seeds)
    env = make_env(net, n, seeds=np.asarray(seeds, dtype=np.uint64))
    names = EV.encounter_names()
    cap = E2E["decision_cap"] + 8
    log = np.zeros((cap, n), dtype=np.uint16)
    steps = np.zeros(n, dtype=np.int64)
    finished = np.zeros(n, dtype=bool)
    import torch

    cur = [None] * n
    removed = [None] * n
    removing = [False] * n
    seen = [None] * n  # unstick: (v1 row digest, action) of the last event decision
    fights, runs = [], [None] * n
    try:
        while not finished.all():
            active = np.nonzero(~finished)[0]
            for i in active:
                row = env.obs[i]
                f = EV.fight_of(row)
                if (cur[i] is None and f is not None and names[f[1]].endswith("_BOSS")
                        and int(row[R.H_PHASE]) == R.PHASE_COMBAT):
                    cur[i] = {"seed": int(seeds[i]), "boss": names[f[1]], "act": f[2] + 1,
                              "floor": int(row[EV.H_FLOOR]), "prefix": EV.pack(log[: steps[i], i]),
                              "sha_v1": R.sha(row), "entry": entry_of(row), "rows": [],
                              "end": None}
            lg = logits_of(net, env, active, dev)
            action = lg.argmax(-1).cpu().numpy()
            if unstick:
                # RLN39: an event option that leaves the state unchanged (Trial's Double Down) is
                # masked when the net picks it twice in a row on the same state.
                for j, i in enumerate(active):
                    row = env.obs[i]
                    if int(row[R.H_PHASE]) != 1:
                        seen[i] = None
                        continue
                    key = (R.sha(row), int(action[j]))
                    if seen[i] == key:
                        lg[j, int(action[j])] = float("-inf")
                        action[j] = int(lg[j].argmax())
                        overrides["unstick"] += 1
                    seen[i] = (key[0], int(action[j]))
            if smith is not None or remove:
                legal = torch.isfinite(lg).cpu().numpy()
                for j, i in enumerate(active):
                    row = env.obs[i]
                    ph = int(row[R.H_PHASE])
                    if (smith is not None and ph == 7 and legal[j, SMITH]
                            and R.i16(row, R.H_HP) >= smith * R.i16(row, R.H_MAXHP)):
                        action[j] = SMITH
                        overrides["smith"] += 1
                    key = (int(row[EV.H_ACT]), int(row[EV.H_FLOOR]))
                    if remove and ph == 9 and legal[j, REMOVE] and removed[i] != key:
                        action[j] = REMOVE
                        removed[i] = key
                        removing[i] = target
                        overrides["remove"] += 1
                    elif removing[i] and ph == 5:
                        k = removal_target(row, legal[j])
                        if k is not None:
                            action[j] = SELECT0 + k
                            overrides["target"] += 1
                        removing[i] = False
            for j, i in enumerate(active):
                if cur[i] is not None:
                    cur[i]["rows"].append(fight_row(env.obs[i], int(action[j])))
            log[steps[active], active] = action
            steps[active] += 1
            acts = np.full(n, HOLD, dtype=np.uint16)
            acts[active] = action
            env.step(acts)
            for i in active:
                if cur[i] is not None:
                    end = fight_end(env, i)
                    if end is not None:
                        cur[i]["end"], cur[i]["final"] = end, final_row(env, i)
                        fights.append(cur[i])
                        cur[i] = None
                if env.done[i]:
                    finished[i] = True
                    runs[i] = {"seed": int(seeds[i]), "hash": EV.action_hash(log[: steps[i], i]),
                               "floor": int(env.floor[i]), "won": int(env.won[i])}
    finally:
        env.close()
    return fights, runs


def cmd_harvest(args):
    dev, net = device_and_net(args.ckpt, args.weights)
    seeds = list(range(args.first, args.first + args.count))
    fights, runs = [], []
    t0 = time.time()
    for k in range(0, len(seeds), args.batch):
        f, r = harvest_batch(net, dev, seeds[k:k + args.batch], args.smith, args.remove, args.target,
                             args.unstick)
        fights += f
        runs += r
        print(f"{k + len(r)}/{len(seeds)} runs  {len(fights)} boss fights  {time.time() - t0:.0f}s",
              flush=True)
        write(args.out, fights, {"kind": "rln36-harvest", "first": args.first, "count": k + len(r),
                                 "partial": True, "runs": runs})
    if args.check:
        _, ev = R.read_jsonl(args.check)
        want = {r["seed"]: r["hash"] for r in ev}
        same = sum(1 for r in runs if want.get(r["seed"]) == r["hash"])
        common = sum(1 for r in runs if r["seed"] in want)
        print(f"check: {same}/{common} run hashes equal {args.check}", flush=True)
    print(f"overrides: {dict(overrides)}", flush=True)
    write(args.out, fights, {"kind": "rln36-harvest", "first": args.first, "count": args.count,
                             "smith": args.smith, "remove": args.remove, "overrides": dict(overrides),
                             "ckpt_sha256": EV.file_sha256(args.ckpt), "weights": args.weights,
                             "runs": runs, "commit": EV.code_commit()})
    return 0


# ---- the fight players (shared by act3 and cf) ----


def potion_first(logits, idx):
    """Greedy, except that a legal potion action (best by logit) is taken first."""
    import torch

    t = torch.as_tensor(table()[: logits.shape[1], 0] == KIND_POTION, device=logits.device)
    pot = logits.masked_fill(~t, float("-inf"))
    has = torch.isfinite(pot).any(-1)
    return torch.where(has, pot.argmax(-1), logits.argmax(-1))


def choose(net, env, idx, mode, dev, gen):
    import torch

    if len(idx) == 0:
        return np.zeros(0, dtype=np.uint16)
    lg = logits_of(net, env, idx, dev)
    a = np.zeros(len(idx), dtype=np.int64)
    mode = np.asarray(mode)
    g = mode == "greedy"
    if g.any():
        a[g] = lg[torch.from_numpy(np.nonzero(g)[0]).to(dev)].argmax(-1).cpu().numpy()
    p = mode == "potion"
    if p.any():
        a[p] = potion_first(lg[torch.from_numpy(np.nonzero(p)[0]).to(dev)], None).cpu().numpy()
    s = mode == "sample"
    if s.any():
        sl = lg[torch.from_numpy(np.nonzero(s)[0]).to(dev)]
        a[s] = torch.multinomial(torch.softmax(sl, -1), 1, generator=gen).squeeze(1).cpu().numpy()
    return a.astype(np.uint16)


# ---- act 3 ----


def cmd_act3(args):
    import torch

    from sts2 import HOLD
    from sts2.ffi import CELL_NAMES

    boss3 = CELL_NAMES.index("BOSS_ACT3")
    dev, net = device_and_net(args.ckpt, args.weights)
    gen = torch.Generator(device=dev)
    gen.manual_seed(36)
    n = args.envs
    spare0 = n - args.spare
    env = make_env(net, n, seed_range=(26_000_000, 27_000_000))
    w = [0] * 6
    w[boss3] = 1
    env.set_practice(slots=args.slots, weights=w, pool=64, max_uses=args.max_uses, min_active=1,
                     capture=True, seed=36)
    names = EV.encounter_names()
    per = 1 + args.samples  # spare envs per traced fight
    free = list(range(spare0, n - per + 1, per))
    mode = np.array(["greedy"] * n, dtype=object)
    held = np.ones(n, dtype=bool)
    held[:spare0] = False
    cur = {}  # env -> open fight record
    owner = {}  # spare env -> (fight id, variant)
    fights, cfs = [], []
    started = np.zeros(n, dtype=bool)  # practice env at its first decision
    by_id = {}
    t0 = last = time.time()
    try:
        while len(fights) < args.fights and time.time() - t0 < args.minutes * 60:
            obs = env.obs
            for i in range(args.slots):
                c = int(env.cell[i])
                row = obs[i]
                f = EV.fight_of(row)
                if (i not in cur and c == boss3 + 1 and f is not None
                        and names[f[1]].endswith("_BOSS") and int(row[R.H_PHASE]) == R.PHASE_COMBAT):
                    rec = {"id": f"p{i}-{int(env.practice_stats()['started'][boss3])}-{len(cfs)}",
                           "boss": names[f[1]], "act": f[2] + 1, "entry": entry_of(row), "rows": [],
                           "end": None, "first": not started[i], "env": i}
                    cur[i] = rec
                    if not started[i] and len(free) and args.samples >= 0:
                        base = free.pop(0)
                        dst = np.arange(base, base + per)
                        env.copy(np.full(per, i), dst)
                        cf = {"id": rec["id"], "boss": rec["boss"], "ends": {}, "hp": {}}
                        cfs.append(cf)
                        by_id[rec["id"]] = cf
                        for k, d in enumerate(dst):
                            v = "potion" if k == 0 else f"s{k}"
                            owner[int(d)] = (cf, v, base)
                            mode[d] = "potion" if k == 0 else "sample"
                            held[d] = False
                    started[i] = True
            live = np.nonzero(~held)[0]
            act = np.full(n, HOLD, dtype=np.uint16)
            for m in ("greedy", "potion", "sample"):
                sel = live[mode[live] == m]
                if len(sel):
                    act[sel] = choose(net, env, sel, [m] * len(sel), dev, gen)
            for i, rec in cur.items():
                rec["rows"].append(fight_row(obs[i], int(act[i])))
            env.step(act)
            for i in list(cur):
                end = fight_end(env, i)
                if end is not None:
                    rec = cur.pop(i)
                    rec["end"], rec["final"] = end, final_row(env, i)
                    fights.append(rec)
                    if rec["id"] in by_id:
                        by_id[rec["id"]]["ends"]["greedy"] = end
                        by_id[rec["id"]]["hp"]["greedy"] = rec["final"][1]
            for d in list(owner):
                end = fight_end(env, d)
                if end is not None:
                    cf, v, base = owner.pop(d)
                    cf["ends"][v] = end
                    cf["hp"][v] = final_row(env, d)[1]
                    held[d] = True
                    mode[d] = "greedy"
                    if not any(b == base for _, _, b in owner.values()):
                        free.append(base)
            for i in range(args.slots):
                if env.done[i]:
                    started[i] = False
            if time.time() - last > 60:
                last = time.time()
                st = env.practice_stats()
                print(f"{time.time() - t0:.0f}s fights {len(fights)} cf {len(cfs)} "
                      f"captured {st['captured'][boss3]} started {st['started'][boss3]}", flush=True)
    finally:
        st = env.practice_stats()
        env.close()
    write(args.out, fights, {"kind": "rln36-act3", "cfs": cfs, "samples": args.samples,
                             "captured": st["captured"][boss3], "started": st["started"][boss3],
                             "ckpt_sha256": EV.file_sha256(args.ckpt), "weights": args.weights})
    print(f"{len(fights)} fights, {len(cfs)} counterfactual sets, captured "
          f"{st['captured'][boss3]}", flush=True)
    return 0


# ---- act-2 counterfactuals ----


def replay(net, entries, per):
    from sts2 import HOLD

    n = len(entries) * per
    seeds = np.asarray([e["seed"] for e in entries for _ in range(per)], dtype=np.uint64)
    env = make_env(net, n, seeds=seeds)
    prefixes = [EV.unpack(e["prefix"]) for e in entries]
    acts = np.full((max(len(p) for p in prefixes), n), HOLD, dtype=np.uint16)
    for f, p in enumerate(prefixes):
        acts[: len(p), f * per] = p
    for t in range(len(acts)):
        env.step(acts[t])
    bad = [f for f in range(len(entries)) if R.sha(env.obs[f * per]) != entries[f]["sha_v1"]]
    if bad:
        raise SystemExit(f"{len(bad)} prefixes reach a different state")
    main = np.arange(len(entries)) * per
    for f in range(len(entries)):
        env.copy(np.full(per - 1, main[f]), main[f] + 1 + np.arange(per - 1))
    return env


def outcome(env, i):
    end = fight_end(env, i)
    if end is None:
        return None
    return end, (final_row(env, i)[1] if end == "won" else 0)


def lookahead_choice(net, env, dev, f_main, spares, k, n, rng):
    """For each fight (main env, its spare envs): the top-k actions, each tried on n reseeded
    copies played out greedily; most wins, then HP, then policy rank."""
    from sts2 import HOLD

    lg = logits_of(net, env, f_main, dev)
    top = lg.topk(min(k, lg.shape[1]), dim=1)
    vals, ids = top.values.cpu().numpy(), top.indices.cpu().numpy()
    choice = {}
    src, dst, info = [], [], []
    for row, m in enumerate(f_main):
        cand = [int(a) for a, v in zip(ids[row], vals[row]) if np.isfinite(v)]
        if len(cand) <= 1:
            choice[m] = cand[0]
            continue
        sp = spares[m]
        for j, a in enumerate(cand):
            for r in range(n):
                d = sp[j * n + r]
                src.append(m)
                dst.append(d)
                info.append((m, j, a, d))
        choice[m] = cand
    if not src:
        return {m: int(c) for m, c in choice.items()}
    env.copy(np.asarray(src), np.asarray(dst), reseed=rng.integers(0, 2**63, len(src), dtype=np.uint64))
    acts = np.full(env.n, HOLD, dtype=np.uint16)
    for m, j, a, d in info:
        acts[d] = a
    env.step(acts)
    res = {}
    running = []
    for m, j, a, d in info:
        o = outcome(env, d)
        if o is None:
            running.append(d)
        else:
            res[d] = o
    steps = 0
    while running and steps < 3000:
        idx = np.asarray(running)
        a = logits_of(net, env, idx, dev).argmax(-1).cpu().numpy()
        acts = np.full(env.n, HOLD, dtype=np.uint16)
        acts[idx] = a
        env.step(acts)
        nxt = []
        for d in running:
            o = outcome(env, d)
            if o is None:
                nxt.append(d)
            else:
                res[d] = o
        running = nxt
        steps += 1
    for d in running:
        res[d] = ("capped", 0)
    score = collections.defaultdict(lambda: [0, 0])
    for m, j, a, d in info:
        e, hp = res[d]
        score[(m, j)][0] += int(e == "won")
        score[(m, j)][1] += hp
    out = {}
    for m, c in choice.items():
        if isinstance(c, int):
            out[m] = c
        else:
            best = max(range(len(c)), key=lambda j: (score[(m, j)][0], score[(m, j)][1], -j))
            out[m] = c[best]
    return out


def cf_chunk(net, dev, entries, samples, k, n, gen, rng):
    """Variants per fight: greedy, potion, samples x sample, and (k > 0) a lookahead main env
    with k*n spares."""
    from sts2 import HOLD

    look = 1 + k * n if k > 0 else 0
    per = 2 + samples + look
    env = replay(net, entries, per)
    N = env.n
    mode = np.array(["greedy"] * N, dtype=object)
    variant = {}
    look_main, spares = [], {}
    for f, e in enumerate(entries):
        b = f * per
        variant[b] = (f, "greedy")
        variant[b + 1] = (f, "potion")
        mode[b + 1] = "potion"
        for s in range(samples):
            variant[b + 2 + s] = (f, f"s{s}")
            mode[b + 2 + s] = "sample"
        if look:
            m = b + 2 + samples
            variant[m] = (f, "look")
            mode[m] = "look"
            look_main.append(m)
            spares[m] = list(range(m + 1, m + look))
    held = np.ones(N, dtype=bool)
    held[list(variant)] = False
    res = [{"id": f"{e['seed']}-{e['floor']}", "boss": e["boss"], "ends": {}, "hp": {}}
           for e in entries]
    while not held.all():
        live = np.nonzero(~held)[0]
        act = np.full(N, HOLD, dtype=np.uint16)
        for m in ("greedy", "potion", "sample"):
            sel = live[mode[live] == m]
            if len(sel):
                act[sel] = choose(net, env, sel, [m] * len(sel), dev, gen)
        lm = [m for m in look_main if not held[m]]
        if lm:
            ch = lookahead_choice(net, env, dev, np.asarray(lm), spares, k, n, rng)
            for m, a in ch.items():
                act[m] = a
        env.step(act)
        for d in live:
            o = outcome(env, d)
            if o is not None:
                f, v = variant[d]
                res[f]["ends"][v], res[f]["hp"][v] = o
                held[d] = True
    env.close()
    return res


def cmd_cf(args):
    import torch

    dev, net = device_and_net(args.ckpt, args.weights)
    gen = torch.Generator(device=dev)
    gen.manual_seed(args.seed)
    rng = np.random.default_rng(args.seed)
    _, fights = R.read_jsonl(args.harvest)
    bosses = args.bosses.split(",")
    sel = []
    for b in bosses:
        sel += [f for f in fights if f["boss"] == b][: args.per]
    out = []
    t0 = time.time()
    for c in range(0, len(sel), args.chunk):
        out += cf_chunk(net, dev, sel[c:c + args.chunk], args.samples, args.k, args.n, gen, rng)
        print(f"{len(out)}/{len(sel)} fights  {time.time() - t0:.0f}s", flush=True)
        write(args.out, out, {"kind": "rln36-cf", "samples": args.samples, "k": args.k,
                              "n": args.n, "harvest": args.harvest, "weights": args.weights})
    return 0


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    h = sub.add_parser("harvest")
    h.add_argument("ckpt")
    h.add_argument("--weights", default="ema0.999")
    h.add_argument("--first", type=int, default=24_000_000)
    h.add_argument("--count", type=int, default=1000)
    h.add_argument("--batch", type=int, default=500)
    h.add_argument("--check")
    h.add_argument("--smith", type=float, help="override: Smith when legal and HP >= this fraction")
    h.add_argument("--remove", action="store_true", help="override: card removal once per shop")
    h.add_argument("--target", action="store_true",
                   help="with --remove: remove a curse, else a Strike, else a Defend (unupgraded first)")
    h.add_argument("--unstick", action="store_true",
                   help="RLN39: mask an event option picked twice in a row on an unchanged state")
    h.add_argument("--out", required=True)
    a = sub.add_parser("act3")
    a.add_argument("ckpt")
    a.add_argument("--weights", default="ema0.999")
    a.add_argument("--envs", type=int, default=1024)
    a.add_argument("--slots", type=int, default=128)
    a.add_argument("--spare", type=int, default=144)
    a.add_argument("--samples", type=int, default=8)
    a.add_argument("--max-uses", type=int, default=4)
    a.add_argument("--fights", type=int, default=80)
    a.add_argument("--minutes", type=float, default=60)
    a.add_argument("--out", required=True)
    c = sub.add_parser("cf")
    c.add_argument("ckpt")
    c.add_argument("--weights", default="ema0.999")
    c.add_argument("--harvest", required=True)
    c.add_argument("--bosses", default=",".join(R.BOSSES))
    c.add_argument("--per", type=int, default=30)
    c.add_argument("--samples", type=int, default=16)
    c.add_argument("--k", type=int, default=3)
    c.add_argument("--n", type=int, default=4)
    c.add_argument("--chunk", type=int, default=15)
    c.add_argument("--seed", type=int, default=0)
    c.add_argument("--out", required=True)
    args = p.parse_args()
    return {"harvest": cmd_harvest, "act3": cmd_act3, "cf": cmd_cf}[args.cmd](args)


if __name__ == "__main__":
    raise SystemExit(main())
