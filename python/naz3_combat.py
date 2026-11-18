"""NAZ3: stronger Necrobinder combat by decision-time search on top of the combat head.

    python python/naz3_combat.py bench                         # the fight-start benchmark
    python python/naz3_combat.py run CKPT --reps R --seed S --out F.npz                # greedy
    python python/naz3_combat.py run CKPT --k 3 --m 8 --reps R --seed S --out F.npz    # search
    python python/naz3_combat.py report BASE.npz [OTHER.npz ...]

**Benchmark** (`bench`): fight starts (gym records, `sts2_rl::human::HumanStart`) sampled from
NAZ1's held-out own-run harvest (`~/.cache/spirebot-data/naz1/own_test.jsonl`: NECRO5 playing
A10 runs), weighted to bosses and elites: per act up to 160 bosses, 120 elites, 80 hallway
fights. Written to `~/.cache/spirebot-data/naz3/bench.jsonl` (never committed).

**run**: every start is played `reps` times in the combat gym (sequential: rollout `k` is built
on seed `seed + k`, so two players run with the same seed meet the same streams until their
moves differ; results are paired by row). Per rollout: HP lost, died, potions used, agent
decisions, searched decisions and deviations from the net's greedy move.

**Search player** (`--k > 1`, `SearchPlayer`): at each decision of a real fight, the net's
top-`k` actions with policy probability >= `pmin` are scored (no search when the top action has
probability >= `pskip` or only one candidate is left). The fight is copied `m` times per
candidate (`Sts2RunEnvs.copy` with `reseed`, RLN28's determinization: the draw pile and the
fight's streams are resampled, the visible state kept); the *same* `m` seeds serve every
candidate (common random numbers, so candidates are compared on the same sampled futures). The
candidate is applied and the copy is played to the fight's end by the net, greedily. Utility of
a rollout: a win scores `1 + hp_w * HP left / max HP + pot_w * potions left`, a death or cap 0.
The best candidate by mean utility is played if it beats the greedy move's mean by more than
`margin`, else the greedy move. `--rooms` limits search to some rooms (the rest play greedily).

Cost: one search costs `k * m` rollouts; all live fights search together, so the net sees
batches of `fights * k * m` rows and the env steps them on the unit's threads.
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

DATA = pathlib.Path(os.path.expanduser("~/.cache/spirebot-data/naz3"))
NAZ1 = pathlib.Path(os.path.expanduser("~/.cache/spirebot-data/naz1"))
ENGINE_KEYS = ("act", "act_id", "encounter", "floor", "hp", "max_hp", "gold", "deck", "relics",
               "potion_slots", "potions")
H_PHASE, H_HP, H_MAXHP, PHASE_COMBAT = 3, 8, 10, 3
BELT_AT, BELT_SLOTS = 480, 6
GPU_CHUNK = int(os.environ.get("NAZ3_GPU_CHUNK", "1536"))
ROOMS = ("hallway", "elite", "boss")
PER_GROUP = {"boss": 160, "elite": 120, "hallway": 80}
COMBAT_AT, C_CREATURES, CREATURE_BYTES, CREATURES = 1750, 24, 60, 8  # `rln19_act2.py`
ROLLOUT_CAP = 600  # agent decisions per rollout; beyond it the rollout scores as a loss


def room_of(r):
    e = r["encounter"]
    return "boss" if e.endswith("_BOSS") else "elite" if e.endswith("_ELITE") else "hallway"


def read_jsonl(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def engine_line(r):
    return json.dumps({k: r[k] for k in ENGINE_KEYS})


def i16_col(obs, at):
    return (obs[:, at].astype(np.int32) | obs[:, at + 1].astype(np.int32) << 8).astype(np.int16)


def belt_counts(obs):
    b = obs[:, BELT_AT:BELT_AT + 2 * BELT_SLOTS].reshape(len(obs), BELT_SLOTS, 2)
    return (b != 0).any(2).sum(1)


_OSTY = None


def enemy_dealt(obs):
    """Fraction of the enemies' max HP (all enemies present, dead ones too; Osty excluded) that
    is gone, per row of a combat obs."""
    global _OSTY
    if _OSTY is None:
        import rln19_act2 as R

        _OSTY = R._vocab("monsters").index("OSTY")
    c = COMBAT_AT + C_CREATURES
    b = obs[:, c:c + CREATURE_BYTES * CREATURES].reshape(len(obs), CREATURES, CREATURE_BYTES)
    b = b.astype(np.int32)
    mon = b[:, :, 4] | b[:, :, 5] << 8
    hp = (b[:, :, 6] | b[:, :, 7] << 8).astype(np.int16)
    mx = (b[:, :, 8] | b[:, :, 9] << 8).astype(np.int16)
    enemy = (b[:, :, 1] == 1) & (mon != _OSTY) & ((mon != 0) | (b[:, :, 0] != 0))
    alive = enemy & (b[:, :, 0] != 0)
    ehp = np.where(alive, np.maximum(hp, 0), 0).sum(1)
    emax = np.where(enemy, mx, 0).sum(1)
    return 1.0 - ehp / np.maximum(emax, 1)


# ---------------------------------------------------------------------------------------------


def cmd_bench(args):
    rng = np.random.default_rng(args.seed)
    recs = read_jsonl(NAZ1 / "own_test.jsonl")
    out = []
    for act in range(3):
        for room in ROOMS:
            pool = [r for r in recs if r["act"] == act and room_of(r) == room]
            n = min(len(pool), PER_GROUP[room])
            out += [pool[i] for i in sorted(rng.choice(len(pool), n, replace=False))]
    order = rng.permutation(len(out))  # any prefix is a balanced subset
    out = [out[i] for i in order]
    DATA.mkdir(parents=True, exist_ok=True)
    with open(DATA / "bench.jsonl", "w") as f:
        for r in out:
            f.write(json.dumps(r) + "\n")
    c = collections.Counter((r["act"], room_of(r)) for r in out)
    print(f"{len(out)} starts: {sorted(c.items())}")
    return 0


# ---------------------------------------------------------------------------------------------


def net_and_dev(ckpt, weights):
    import torch

    import e2e_eval as EV

    torch.set_num_threads(2)
    dev = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    if dev.type == "cuda":  # NAZ3's share: <= 4 GB of the shared GPU
        total = torch.cuda.get_device_properties(0).total_memory
        gb = float(os.environ.get("NAZ3_GPU_GB", "4"))
        torch.cuda.set_per_process_memory_fraction(min(1.0, gb * 2**30 / total))
    net = EV.load_policy(ckpt, weights, dev)
    if os.environ.get("NAZ3_COMPILE") == "1" and dev.type == "cuda":  # ~1.8x rows/s, float noise
        from sts2.e2e import encode as E

        net.enable_compile()
        E.enable_compile()
    return net, dev


_WIDTH = {}  # id(env) -> each env's token width at its last net call (the batching sort key)


def logits(net, env, idx, dev):
    """Masked float32 logits (torch, on `dev`) of env rows `idx`, in GPU-sized chunks. Rows are
    sorted by their token width at the env's last call, so a chunk pads little (the net trims a
    batch to its widest row): about 1.5x the rows/s of unsorted chunks."""
    import torch

    from sts2.e2e import encode as E

    import rln36_boss as B

    if not len(idx):
        return None
    idx = np.asarray(idx)
    w = _WIDTH.get(id(env))
    if w is None or len(w) != env.n:
        w = _WIDTH[id(env)] = np.zeros(env.n, dtype=np.int32)
    order = np.argsort(w[idx], kind="stable")
    f = B.flags(net)
    out = [None] * ((len(idx) + GPU_CHUNK - 1) // GPU_CHUNK)
    for c, k in enumerate(range(0, len(idx), GPU_CHUNK)):
        rows = idx[order[k:k + GPU_CHUNK]]
        obs = torch.from_numpy(env.obs[rows]).to(dev)
        masks = torch.from_numpy(env.masks.view(np.int64)[rows]).to(dev)
        with torch.no_grad():
            tok = E.encode(obs, masks, **f)
            with torch.autocast(dev.type, dtype=torch.bfloat16, enabled=dev.type == "cuda"):
                lg, _, _ = net.policy(tok)
            lg = lg.float().nan_to_num(nan=float("-inf"))
            legal = E.mask_bits(masks, discard=f["discard"])[:, : lg.shape[1]]
            out[c] = lg.masked_fill(~legal, float("-inf"))
        w[rows] = tok["width"].cpu().numpy()
    lg = torch.cat(out)
    inv = torch.from_numpy(np.argsort(order)).to(lg.device)
    return lg[inv]


class SearchPlayer:
    """Top-k x m common-random-number rollouts to the fight's end, greedy rollout policy (module
    docs). `choose` returns the actions of the fights `idx` (rows of `lg`)."""

    def __init__(self, k=3, m=8, pmin=0.05, pskip=0.95, margin=0.0, hp_w=1.0, pot_w=0.05,
                 rooms=ROOMS, seed=0, loss_w=0.0, temp=0.0, crn=True):
        self.k, self.m, self.pmin, self.pskip, self.margin = k, m, pmin, pskip, margin
        self.hp_w, self.pot_w, self.loss_w = hp_w, pot_w, loss_w
        self.rooms = set(rooms)
        self.temp, self.crn = temp, crn  # RLN28's setting: temp 1, crn False
        self.gen = None
        self.base = 0  # the first scratch env; the caller sets it
        self.rng = np.random.default_rng(seed)
        self.stats = collections.Counter()

    def scratch(self, fights):
        """Scratch envs needed for main envs `0..fights` (put them at `self.base`)."""
        return fights * self.k * self.m

    def act(self, net, env, dev, idx, lg=None):
        """The NAZ2 entry point: actions (numpy uint16) for main envs `idx`, each at a combat
        decision. Main env ids must be below `fights` of `scratch(fights)`; scratch envs
        `base..base + scratch(fights)` of the same `Sts2RunEnvs` are overwritten (they are
        held, i.e. stepped with HOLD, while not in use; their auto-reset runs are harmless).
        `lg`: the rows' logits if the caller has them. Rooms outside `self.rooms` play greedy;
        the room is read from the obs (`e2e_eval.fight_of`)."""
        import e2e_eval as EV

        idx = np.asarray(idx)
        if lg is None:
            lg = logits(net, env, idx, dev)
        names = EV.encounter_names()
        ok = np.zeros(len(idx), dtype=bool)
        for r, i in enumerate(idx):
            fight = EV.fight_of(env.obs[i])
            ok[r] = fight is not None and room_of({"encounter": names[fight[1]]}) in self.rooms
        return self.choose(net, env, dev, idx, lg, ok)

    def utility(self, env, d, dealt):
        """Utility of rollout envs `d` that just finished (see the module docs)."""
        from sts2.env import RUN_ENDS

        obs = env.obs[d]
        done = env.done[d].astype(bool)
        if done.any():
            obs = obs.copy()
            obs[done] = env.final_obs[d[done]]
        ends = np.array([RUN_ENDS[e] for e in env.end[d]])
        won = ~done | np.isin(ends, ("gym_end", "victory"))
        hp = np.maximum(0, i16_col(obs, H_HP)).astype(np.float64)
        mx = np.maximum(1, i16_col(obs, H_MAXHP)).astype(np.float64)
        win = 1.0 + self.hp_w * hp / mx + self.pot_w * belt_counts(obs)
        return np.where(won, win, self.loss_w * dealt[d])

    def rollouts(self, net, env, dev, src, dst, act, seeds, slots, per):
        """Copies main envs `src` into scratch envs `dst` (reseeded with `seeds`), applies `act`
        and plays every copy to its fight's end with the net (greedy, or sampled at `temp`).
        Returns the utility per env (indexed by env id) and the rollout net rows per main slot
        (scratch env `d` belongs to slot `(d - base) // per`)."""
        import torch

        from sts2 import HOLD

        F = self.base
        per_slot = np.zeros(slots, dtype=np.int64)
        env.copy(src, dst, reseed=seeds)
        acts = np.full(env.n, HOLD, dtype=np.uint16)
        acts[dst] = act
        env.step(acts)
        util = np.zeros(env.n)
        dealt = np.zeros(env.n)  # the enemies' HP gone at a rollout's last combat decision
        dealt[dst] = enemy_dealt(env.obs[src])
        running = dst
        steps = 0
        while len(running):
            fin = env.done[running].astype(bool) | (env.obs[running, H_PHASE] != PHASE_COMBAT)
            if (~fin).any():
                dealt[running[~fin]] = enemy_dealt(env.obs[running[~fin]])
            if fin.any():
                util[running[fin]] = self.utility(env, running[fin], dealt)
            running = running[~fin]
            if not len(running) or steps >= ROLLOUT_CAP:
                break
            lr = logits(net, env, running, dev)
            if self.temp > 0:
                if self.gen is None:
                    self.gen = torch.Generator(device=dev)
                    self.gen.manual_seed(int(self.rng.integers(2**62)))
                a = torch.multinomial(torch.softmax(lr / self.temp, -1), 1,
                                      generator=self.gen).squeeze(1)
            else:
                a = lr.argmax(-1)
            acts = np.full(env.n, HOLD, dtype=np.uint16)
            acts[running] = a.cpu().numpy()
            env.step(acts)
            steps += 1
            self.stats["rollout_steps"] += len(running)
            np.add.at(per_slot, (running - F) // per, 1)
        self.stats["capped"] += len(running)
        return util, per_slot

    def choose(self, net, env, dev, mains, lg, room_ok):
        """Actions (numpy) for main envs `mains`; `lg` their logits; `room_ok` which may search."""
        import torch

        p = torch.softmax(lg, -1)
        top = torch.topk(p, min(self.k, p.shape[1]), dim=1)
        pv, pa = top.values.cpu().numpy(), top.indices.cpu().numpy()
        choice = pa[:, 0].copy()
        cand = (pv >= self.pmin) & (pv > 0)  # p 0: an illegal action
        cand[:, 0] = True
        ncand = cand.sum(1)
        search = room_ok & (ncand > 1) & (pv[:, 0] < self.pskip)
        self.stats["decisions"] += len(mains)
        self.last_searched = search.astype(np.int32)
        self.last_rows = np.zeros(len(mains), dtype=np.int64)  # rollout net rows per main row
        self.last_cand = np.where(cand, pa, -1)  # candidates (-1: none) and their mean utility
        self.last_q = np.full(cand.shape, np.nan, dtype=np.float32)
        self.last_u = np.full((*cand.shape, self.m), np.nan, dtype=np.float16)  # per sample
        if not search.any():
            return choice
        rows = np.nonzero(search)[0]
        self.stats["searched"] += len(rows)
        k, m, F = self.k, self.m, self.base
        # rollout env of (row r, candidate j, sample s): F + (main slot * k + j) * m + s
        src, dst, act, seeds = [], [], [], []
        for r in rows:
            f = mains[r]
            ss = self.rng.integers(0, 2**63, size=m, dtype=np.uint64)
            for j in range(k):
                if not cand[r, j]:
                    continue
                if not self.crn:
                    ss = self.rng.integers(0, 2**63, size=m, dtype=np.uint64)
                base = F + (f * k + j) * m
                src += [f] * m
                dst += list(range(base, base + m))
                act += [pa[r, j]] * m
                seeds.append(ss)
        src, dst = np.asarray(src), np.asarray(dst)
        util, per_slot = self.rollouts(net, env, dev, src, dst, np.asarray(act),
                                       np.concatenate(seeds), int(mains.max()) + 1, k * m)
        self.last_rows = per_slot[mains]
        for r in rows:
            f = mains[r]
            means = np.full(k, -np.inf)
            for j in range(k):
                if cand[r, j]:
                    base = F + (f * k + j) * m
                    means[j] = util[base:base + m].mean()
                    self.last_q[r, j] = means[j]
                    self.last_u[r, j] = util[base:base + m]
            best = int(np.argmax(means))
            if best != 0 and means[best] - means[0] > self.margin:
                choice[r] = pa[r, best]
                self.stats["deviated"] += 1
        return choice


def play(net, dev, recs, reps, seed, fights, player=None, threads=4, log_every=60,
         on_step=None, on_end=None, play_temp=0.0):
    """Plays every start `reps` times (module docs); a dict of per-rollout arrays.
    `on_step(env, idx, k_of, lg, a)` sees each decision before it is played (rows `idx` of the
    env, rollout numbers `k_of`, logits, actions); `on_end(k, won, hp_left, max_hp)` each end."""
    import rln36_boss as B
    from sts2 import HOLD
    from sts2.env import RUN_ENDS

    total = len(recs) * reps
    DATA.mkdir(parents=True, exist_ok=True)
    tmp = DATA / f"gym-{os.getpid()}.jsonl"
    tmp.write_text("".join((engine_line(r) + "\n") * reps for r in recs))
    F = min(fights, total)
    N = F + (player.scratch(F) if player else 0)
    if player:
        player.base = F
    B.THREADS = threads
    env = B.make_env(net, N, seed_range=(0, 1), cycle=True, **getattr(player, "env_kw", {}))
    res = {k: np.zeros(total, dtype=t) for k, t in (
        ("hp_lost", np.int16), ("died", np.uint8), ("potions", np.int8), ("decisions", np.int32),
        ("searched", np.int32), ("deviated", np.int32), ("rows", np.int64), ("bad", np.uint8))}
    entry_belt = np.zeros(total, dtype=np.int8)
    gen = None
    rooms = np.array([room_of(r) for r in recs for _ in range(reps)])
    try:
        assert env.load_gym(tmp) == total
        env.set_gym(first=0, slots=F, sequential=True, seed=seed)
        env.reset_all()
        holds = np.arange(F)
        nxt = F

        def check(i):
            k = holds[i]
            line = env.start_record(int(i))
            r = recs[k // reps]
            got = json.loads(line) if line else {}
            if got.get("encounter") != r["encounter"] or len(got.get("deck", ())) != len(r["deck"]):
                res["bad"][k] = 1
            entry_belt[k] = belt_counts(env.obs[i:i + 1])[0]

        for i in range(F):
            check(i)
        live = np.ones(F, dtype=bool)
        t0, steps, done_n, t_log = time.time(), 0, 0, time.time()
        while live.any():
            idx = np.nonzero(live)[0]
            lg = logits(net, env, idx, dev)
            k_of = holds[idx]
            if player is None and play_temp > 0:  # CB1: sample the net's policy at temperature T
                import torch

                if gen is None:
                    gen = torch.Generator(device=lg.device)
                    gen.manual_seed(seed)
                a = torch.multinomial(torch.softmax(lg / play_temp, -1), 1,
                                      generator=gen).squeeze(1).cpu().numpy()
            elif player is None:
                a = lg.argmax(-1).cpu().numpy()
            else:
                a = player.choose(net, env, dev, idx, lg, np.isin(rooms[k_of], list(player.rooms)))
                res["deviated"][k_of] += a != lg.argmax(-1).cpu().numpy()
                res["searched"][k_of] += player.last_searched
                res["rows"][k_of] += player.last_rows
            res["decisions"][k_of] += 1
            if on_step is not None:
                on_step(env, idx, k_of, lg, a)
            acts = np.full(N, HOLD, dtype=np.uint16)
            acts[idx] = a
            env.step(acts)
            steps += 1
            for i in idx[env.done[idx].astype(bool)]:  # ascending: the env's assign order
                k = holds[i]
                end = RUN_ENDS[env.end[i]]
                hp0 = recs[k // reps]["hp"]
                if end in ("gym_end", "victory"):
                    fo = env.final_obs[i:i + 1]
                    res["hp_lost"][k] = hp0 - int(i16_col(fo, H_HP)[0])
                    res["potions"][k] = entry_belt[k] - belt_counts(fo)[0]
                    if on_end is not None:
                        on_end(k, True, int(i16_col(fo, H_HP)[0]), int(i16_col(fo, H_MAXHP)[0]))
                else:
                    res["hp_lost"][k], res["died"][k] = hp0, 1
                    if on_end is not None:
                        on_end(k, False, 0, recs[k // reps]["max_hp"])
                done_n += 1
                if nxt < total:
                    holds[i] = nxt
                    nxt += 1
                    check(i)
                else:
                    live[i] = False
            if time.time() - t_log > log_every:
                t_log = time.time()
                el = t_log - t0
                print(f"  step {steps}: {done_n}/{total} fights, {el:.0f}s, "
                      f"{res['decisions'].sum() / el:.1f} decisions/s"
                      + (f" {dict(player.stats)}" if player else ""), flush=True)
    finally:
        env.close()
        tmp.unlink(missing_ok=True)
    res["wall_s"] = time.time() - t0
    if res["bad"].mean() > 0.001:
        raise SystemExit(f"{res['bad'].sum()} rollouts did not hold the record they should")
    return res


def cmd_run(args):
    net, dev = net_and_dev(args.ckpt, args.weights)
    recs = read_jsonl(args.bench)
    if args.only:
        recs = [r for r in recs if room_of(r) in args.only.split(",")]
    if args.limit:
        recs = recs[:args.limit]
    player = None
    if args.k > 1:
        player = SearchPlayer(args.k, args.m, args.pmin, args.pskip, args.margin, args.hp_w,
                              args.pot_w, args.rooms.split(","), args.seed, args.loss_w,
                              args.temp, not args.no_crn)
    t0 = time.time()
    res = play(net, dev, recs, args.reps, args.seed, args.fights, player, args.threads,
               play_temp=args.play_temp)
    meta = dict(ckpt=str(args.ckpt), weights=args.weights, reps=args.reps, seed=args.seed,
                bench=str(args.bench), k=args.k, m=args.m, pmin=args.pmin, pskip=args.pskip,
                margin=args.margin, hp_w=args.hp_w, pot_w=args.pot_w, rooms=args.rooms,
                loss_w=args.loss_w, only=args.only, temp=args.temp, crn=not args.no_crn,
                fights=args.fights, threads=args.threads, play_temp=args.play_temp, wall_s=time.time() - t0,
                stats=dict(player.stats) if player else {})
    np.savez(args.out, ids=np.array([r["id"] for r in recs]),
             act=np.array([r["act"] for r in recs]), room=np.array([room_of(r) for r in recs]),
             meta=json.dumps(meta), **{k: v.reshape(len(recs), args.reps)
                                       for k, v in res.items() if k != "wall_s"})
    d = res["decisions"].sum()
    print(f"wrote {args.out}: {len(recs)} x {args.reps} in {meta['wall_s']:.0f}s, {d} decisions, "
          f"{1000 * meta['wall_s'] / d:.1f} ms/decision; {meta['stats']}")
    return 0


# ---------------------------------------------------------------------------------------------


def load(path):
    z = np.load(path)
    return {k: z[k] for k in z.files} | {"meta": json.loads(str(z["meta"]))}


def groups(z):
    out = []
    for room in ("boss", "elite", "hallway"):
        for act in (0, 1, 2, None):
            sel = (z["room"] == room) & ((z["act"] == act) if act is not None else True)
            out.append((f"{room} {'all' if act is None else f'act{act + 1}'}", sel))
    return out


def cmd_report(args):
    """Deaths % (± s.e. over rollouts) and mean HP lost per group; files are matched to the
    first (the baseline) by start id, so a file may cover a subset of the bench."""
    zs = [load(p) for p in args.files]
    base = zs[0]
    print("| group | starts | " + " | ".join(
        f"{pathlib.Path(p).stem} deaths % (HP lost)" for p in args.files) + " |")
    print("|---|---:|" + "---:|" * len(zs))
    for name, sel in groups(base):
        ids = set(base["ids"][sel])
        cells, n = [], int(sel.sum())
        db = None
        for z in zs:
            rows = np.array([i in ids for i in z["ids"]], dtype=bool)
            if not rows.any():
                cells.append("-")
                continue
            d = z["died"][rows].astype(float)
            h = z["hp_lost"][rows].astype(float)
            cell = f"{100 * d.mean():.1f} ±{100 * d.std() / np.sqrt(d.size):.1f} ({h.mean():.1f})"
            if db is None:
                db = d.mean()
            else:
                cell += f" [{100 * (d.mean() - db) / max(db, 1e-9):+.0f}% rel]"
            cells.append(cell)
        print(f"| {name} | {n} | " + " | ".join(cells) + " |")
    for p, z in zip(args.files, zs):
        m = z["meta"]
        d = int(z["decisions"].sum())
        print(f"{pathlib.Path(p).stem}: k={m['k']} m={m['m']} loss_w={m.get('loss_w', 0)} "
              f"pskip={m['pskip']} wall {m['wall_s']:.0f}s, {d} decisions, "
              f"{1000 * m['wall_s'] / d:.1f} ms/decision, potions/fight {z['potions'].mean():.2f}, "
              f"stats {m['stats']}")
    return 0


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    b = sub.add_parser("bench")
    b.add_argument("--seed", type=int, default=0)
    r = sub.add_parser("run")
    r.add_argument("ckpt")
    r.add_argument("--weights", default="ema0.999")
    r.add_argument("--bench", default=str(DATA / "bench.jsonl"))
    r.add_argument("--reps", type=int, default=4)
    r.add_argument("--seed", type=int, default=7_000_000)
    r.add_argument("--limit", type=int, default=0)
    r.add_argument("--fights", type=int, default=256, help="real fights played at once")
    r.add_argument("--threads", type=int, default=4)
    r.add_argument("--k", type=int, default=1, help="1: greedy (no search)")
    r.add_argument("--m", type=int, default=8)
    r.add_argument("--pmin", type=float, default=0.05)
    r.add_argument("--pskip", type=float, default=0.95)
    r.add_argument("--margin", type=float, default=0.0)
    r.add_argument("--hp-w", type=float, default=1.0)
    r.add_argument("--pot-w", type=float, default=0.05)
    r.add_argument("--rooms", default=",".join(ROOMS))
    r.add_argument("--loss-w", type=float, default=0.0,
                   help="a lost rollout scores loss_w x the enemies' HP fraction it took")
    r.add_argument("--temp", type=float, default=0.0, help="rollout temperature (0: greedy)")
    r.add_argument("--play-temp", type=float, default=0.0,
                   help="CB1: sample the played moves at this temperature (0: greedy)")
    r.add_argument("--no-crn", action="store_true", help="fresh seeds per candidate (RLN28)")
    r.add_argument("--only", default="", help="bench rooms to play (e.g. boss,elite); all if empty")
    r.add_argument("--out", required=True)
    q = sub.add_parser("report")
    q.add_argument("files", nargs="+")
    args = p.parse_args()
    return {"bench": cmd_bench, "run": cmd_run, "report": cmd_report}[args.cmd](args)


if __name__ == "__main__":
    raise SystemExit(main())
