"""CB2: the combat search in Rust (`sts2_rl::Search`, `crates/sts2-rl/src/run_envs/search.rs`),
driven from Python: the tree, the determinized copies and their engine steps run on the
engine's threads; Python only evaluates each round's leaves with the net, all fights at once.

    python python/cb2_search.py run CKPT --sims 64 --m 8 --world paired --no-gumbel --pot-w 0.10 \
        --bench F.jsonl --only boss --reps 1 --fights 1024 --out F.npz
    python python/cb2_search.py gen CKPT --starts F.jsonl --count N --seed S --out DIR
    python python/cb2_search.py identity CKPT --fights 16          # Rust vs cb1_mcts, same moves?
    python python/cb2_search.py speed CKPT --fights 1024 --limit 2048

`RustMCTSPlayer` has `cb1_mcts.MCTSPlayer`'s `choose` interface and plays its rules (leaf =
value). The random numbers the Python player draws (Gumbel noise, world seeds) are drawn the
same way here and handed to Rust, so with the same net outputs both players take the same moves
(`identity`: both evaluated by one deterministic, memoised CPU evaluator). Fresh worlds draw
their seeds in Rust (not identical to the Python player's).
"""

import argparse
import collections
import ctypes
import json
import math
import os
import pathlib
import sys
import threading
import time

import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import cb1_mcts as M  # noqa: E402
import naz3_combat as C  # noqa: E402

WORLDS = {"fresh": 0, "paired": 1, "one": 2, "exact": 3}
STAT_KEYS = ("searched", "reused", "dropped_sims", "illegal_sim", "sim_steps", "sim_ends",
             "leaf_evals", "dead_end", "sims", "rounds")


class SearchConfig(ctypes.Structure):
    _fields_ = [("sims", ctypes.c_uint32), ("m", ctypes.c_uint32), ("wave", ctypes.c_uint32),
                ("depth", ctypes.c_uint32), ("threads", ctypes.c_uint32),
                ("world", ctypes.c_uint32), ("keep_tree", ctypes.c_uint32),
                ("reserved", ctypes.c_uint32), ("c_puct", ctypes.c_double),
                ("c_visit", ctypes.c_double), ("c_scale", ctypes.c_double),
                ("hp_coef", ctypes.c_double), ("pot_w", ctypes.c_double)]


def _declare(lib):
    c = ctypes
    if getattr(lib, "_cb2_declared", False):
        return
    vp, sz = c.c_void_p, c.c_size_t
    pd, pu64, pu32 = c.POINTER(c.c_double), c.POINTER(c.c_uint64), c.POINTER(c.c_uint32)
    sigs = {
        "sts2_search_new": ([vp, c.POINTER(SearchConfig)], vp),
        "sts2_search_free": ([vp], None),
        "sts2_search_begin": ([vp, vp, pu32, sz, sz, pd, pd, pd, pu64, sz], c.c_int32),
        "sts2_search_run": ([vp, c.POINTER(c.c_uint8), pu64, c.POINTER(c.c_uint8), sz], c.c_int32),
        "sts2_search_feed": ([vp, pd, pd, pd, sz, sz], c.c_int32),
        "sts2_search_results": ([vp, c.POINTER(c.c_uint16), pd, c.POINTER(c.c_int16),
                                 c.POINTER(c.c_float), sz, sz], c.c_int32),
        "sts2_search_after_step": ([vp, vp, pu32, sz], c.c_int32),
        "sts2_search_stats": ([vp, pu64], c.c_int32),
    }
    for name, (args, res) in sigs.items():
        f = getattr(lib, name)
        f.argtypes, f.restype = args, res
    lib._cb2_declared = True


def _p(a, t):
    return a.ctypes.data_as(ctypes.POINTER(t))


class RustSearch:
    """The ctypes handle of a `sts2_rl::Search` over run batch `env` (module docs)."""

    def __init__(self, env, cap, **cfg):
        from sts2 import ffi

        self.env, self.lib = env, env.lib
        _declare(self.lib)
        c = SearchConfig(**cfg)
        self.ptr = self.lib.sts2_search_new(env._ptr, ctypes.byref(c))
        if not self.ptr:
            raise RuntimeError(ffi.last_error(self.lib))
        self.cap = cap
        self.obs = np.zeros((cap, env.obs.shape[1]), dtype=np.uint8)
        self.masks = np.zeros((cap, env.masks.shape[1]), dtype=np.uint64)
        self.privs = np.zeros((cap, env.priv.shape[1]), dtype=np.uint8)

    def _check(self, code):
        from sts2 import ffi

        if code < 0:
            raise RuntimeError(f"search: {code} {ffi.last_error(self.lib)}")
        return code

    def begin(self, mains, logits, prior, gumbel, seeds):
        mains = np.ascontiguousarray(mains, dtype=np.uint32)
        logits = np.ascontiguousarray(logits, dtype=np.float64)
        prior = np.ascontiguousarray(prior, dtype=np.float64)
        seeds = np.ascontiguousarray(seeds, dtype=np.uint64)
        g = None if gumbel is None else np.ascontiguousarray(gumbel, dtype=np.float64)
        n, width = logits.shape
        self._check(self.lib.sts2_search_begin(
            self.ptr, self.env._ptr, _p(mains, ctypes.c_uint32), n, width,
            _p(logits, ctypes.c_double), _p(prior, ctypes.c_double),
            None if g is None else _p(g, ctypes.c_double), _p(seeds, ctypes.c_uint64),
            seeds.shape[1]))
        self.n, self.width = n, width

    def run(self):
        return self._check(self.lib.sts2_search_run(
            self.ptr, _p(self.obs, ctypes.c_uint8), _p(self.masks, ctypes.c_uint64),
            _p(self.privs, ctypes.c_uint8), self.cap))

    def feed(self, logits, prior, values):
        logits = np.ascontiguousarray(logits, dtype=np.float64)
        prior = np.ascontiguousarray(prior, dtype=np.float64)
        values = np.ascontiguousarray(values, dtype=np.float64)
        self._check(self.lib.sts2_search_feed(
            self.ptr, _p(logits, ctypes.c_double), _p(prior, ctypes.c_double),
            _p(values, ctypes.c_double), len(values), logits.shape[1]))

    def results(self, top=M.TOPN):
        n = self.n
        act = np.zeros(n, dtype=np.uint16)
        vis = np.zeros(n, dtype=np.float64)
        ta = np.zeros((n, top), dtype=np.int16)
        tp = np.zeros((n, top), dtype=np.float32)
        self._check(self.lib.sts2_search_results(
            self.ptr, _p(act, ctypes.c_uint16), _p(vis, ctypes.c_double),
            _p(ta, ctypes.c_int16), _p(tp, ctypes.c_float), top, n))
        return act, vis, ta, tp

    def after_step(self, mains):
        mains = np.ascontiguousarray(mains, dtype=np.uint32)
        self._check(self.lib.sts2_search_after_step(self.ptr, self.env._ptr,
                                                    _p(mains, ctypes.c_uint32), len(mains)))

    def stats(self):
        out = np.zeros(len(STAT_KEYS), dtype=np.uint64)
        self._check(self.lib.sts2_search_stats(self.ptr, _p(out, ctypes.c_uint64)))
        return dict(zip(STAT_KEYS, (int(x) for x in out)))

    def close(self):
        if self.ptr:
            self.lib.sts2_search_free(self.ptr)
            self.ptr = None


def softmax_rows(z):
    """`cb1_mcts.softmax` per row (the same numpy operations, so the same bits)."""
    z = np.where(np.isfinite(z), z, -np.inf)
    e = np.exp(z - z.max(1, keepdims=True))
    return e / e.sum(1, keepdims=True)


def eval_rows(net, obs, masks, priv, dev, vnet=None):
    """`cb1_mcts.eval_net` on arrays: masked logits (float64) and values of the rows."""
    import rln36_boss as B
    import torch
    from sts2.e2e import encode as E

    f = B.flags(net)
    lgs, vs = [], []
    for k in range(0, len(obs), C.GPU_CHUNK):
        sl = slice(k, k + C.GPU_CHUNK)
        o = torch.from_numpy(obs[sl]).to(dev, non_blocking=True)
        mk = torch.from_numpy(masks[sl].view(np.int64)).to(dev, non_blocking=True)
        pv = torch.from_numpy(priv[sl]).to(dev, non_blocking=True)
        with torch.no_grad():
            tok = E.encode(o, mk, **f)
            with torch.autocast(dev.type, dtype=torch.bfloat16, enabled=dev.type == "cuda"):
                lg, h, valid = net.policy(tok)
                ptok = E.encode_priv(pv)
                if vnet is not None:
                    _, h, valid = vnet.policy(tok)
                    v = vnet.value(h, valid, ptok, tok["combat"])
                else:
                    v = net.value(h, valid, ptok, tok["combat"])
            lg = lg.float().nan_to_num(nan=float("-inf"))
            legal = E.mask_bits(mk, discard=f["discard"])[:, : lg.shape[1]]
            lgs.append(lg.masked_fill(~legal, float("-inf")))
            vs.append(v.float())
    import torch as T

    lg = T.cat(lgs).cpu().numpy().astype(np.float64)  # one sync for the round
    v = T.cat(vs).cpu().numpy().astype(np.float64)
    return lg, v


def eval_rows_sorted(net, obs, masks, priv, dev, vnet=None):
    """`eval_rows` with the rows run through the net in token-width order (the net trims a batch
    to its widest row, so sorted chunks pad little); results come back in the input order."""
    import rln36_boss as B
    import torch
    from sts2.e2e import encode as E

    f = B.flags(net)
    n = len(obs)
    o = torch.from_numpy(obs).to(dev, non_blocking=True)
    mk = torch.from_numpy(masks.view(np.int64)).to(dev, non_blocking=True)
    pv = torch.from_numpy(priv).to(dev, non_blocking=True)
    with torch.no_grad():
        ws = []
        for k in range(0, n, C.GPU_CHUNK):
            ws.append(E.encode(o[k:k + C.GPU_CHUNK], mk[k:k + C.GPU_CHUNK], **f)["width"])
        order = torch.argsort(torch.cat(ws), stable=True)
        o, mk, pv = o[order], mk[order], pv[order]
        lgs, vs = [], []
        for k in range(0, n, C.GPU_CHUNK):
            sl = slice(k, k + C.GPU_CHUNK)
            tok = E.encode(o[sl], mk[sl], **f)
            with torch.autocast(dev.type, dtype=torch.bfloat16, enabled=dev.type == "cuda"):
                lg, h, valid = net.policy(tok)
                ptok = E.encode_priv(pv[sl])
                if vnet is not None:
                    _, h, valid = vnet.policy(tok)
                    v = vnet.value(h, valid, ptok, tok["combat"])
                else:
                    v = net.value(h, valid, ptok, tok["combat"])
            lg = lg.float().nan_to_num(nan=float("-inf"))
            legal = E.mask_bits(mk[sl], discard=f["discard"])[:, : lg.shape[1]]
            lgs.append(lg.masked_fill(~legal, float("-inf")))
            vs.append(v.float())
        w = max(x.shape[1] for x in lgs)
        lg = torch.cat([torch.nn.functional.pad(x, (0, w - x.shape[1]), value=float("-inf"))
                        for x in lgs])
        inv = torch.empty_like(order)
        inv[order] = torch.arange(n, device=order.device)
        lg, v = lg[inv], torch.cat(vs)[inv]
    return lg.cpu().numpy().astype(np.float64), v.cpu().numpy().astype(np.float64)


class RustMCTSPlayer:
    """`cb1_mcts.MCTSPlayer` (leaf = value) with the search in Rust; same `choose` interface."""

    def __init__(self, sims=64, m=16, wave=16, c_puct=1.25, c_visit=50.0, c_scale=0.1, depth=40,
                 hp_coef=0.3, pot_w=0.03, gumbel=True, seed=0, rooms=C.ROOMS, keep_tree=True,
                 world="fresh", vnet=None, threads=12, evaluator=None, lanes=2):
        self.cfg = dict(sims=sims, m=m, wave=wave, depth=depth, threads=threads,
                        world=WORLDS[world], keep_tree=int(keep_tree), c_puct=c_puct,
                        c_visit=c_visit, c_scale=c_scale, hp_coef=hp_coef, pot_w=pot_w)
        self.sims, self.m, self.world, self.gumbel = sims, m, world, gumbel
        self.rooms = set(rooms)
        self.vnet = vnet
        self.rng = np.random.default_rng(seed)
        self.stats = collections.Counter()
        self.base = 0
        self.env_kw = dict(priv=True)
        self.lanes = lanes  # Searches over disjoint mains (main % lanes): one's tree work in Rust
        self.searches = None  # overlaps another's GPU eval (ctypes drops the GIL)
        self.gpu = threading.Lock()
        self.evaluator = evaluator  # (net, obs, masks, priv, dev) -> (logits, values)
        self.eval_s = 0.0
        self.last_tgt_a = self.last_tgt_p = None

    def scratch(self, fights):
        return 0

    def _eval(self, net, obs, masks, priv, dev):
        with self.gpu:
            return self._eval_locked(net, obs, masks, priv, dev)

    def _eval_locked(self, net, obs, masks, priv, dev):
        t = time.perf_counter()
        if self.evaluator is not None:
            out = self.evaluator(net, obs, masks, priv, dev)
        else:
            out = eval_rows_sorted(net, obs, masks, priv, dev, self.vnet)
            dump = os.environ.get("CB2_DUMP")
            if dump and len(obs) > 2000 and not os.path.exists(dump):
                np.savez(dump, obs=obs, masks=masks, priv=priv)
        self.eval_s += time.perf_counter() - t
        return out

    def choose(self, net, env, dev, mains, lg, room_ok):
        from sts2 import HOLD

        t0 = time.perf_counter()
        lgn = lg.float().cpu().numpy().astype(np.float64)
        n, width = lgn.shape
        greedy = lgn.argmax(1)
        dead = ~np.isfinite(lgn).any(1)
        choice = greedy.copy()
        choice[dead] = HOLD
        self.stats["empty_rows"] += int(dead.sum())
        safe = np.where(dead[:, None], 0.0, lgn)
        P = np.exp(safe - safe.max(1, keepdims=True))
        P /= P.sum(1, keepdims=True)
        top = np.argsort(-P, 1)[:, :M.TOPN]
        self.last_tgt_a = top.astype(np.int16)
        self.last_tgt_p = np.take_along_axis(P, top, 1).astype(np.float32)
        nlegal = np.isfinite(lgn).sum(1)
        search = room_ok & (nlegal > 1) & ~dead
        if dead.any():  # as `cb1_mcts`: that step must be the real one
            self.stats["skipped_steps"] += 1
            search[:] = False
        self.last_searched = search.astype(np.int32)
        self.last_rows = np.zeros(n, dtype=np.int64)
        self.stats["decisions"] += n
        if not search.any():
            return choice
        if self.searches is None:
            self.searches = [RustSearch(env, cap=env.n * self.cfg["wave"], **self.cfg)
                             for _ in range(self.lanes)]
        rows = np.nonzero(search)[0]
        # the Python player's draws, in its order: per row Gumbel noise, then world seeds
        gum = np.zeros((len(rows), width)) if self.gumbel else None
        per = 1 if self.world == "one" else self.sims
        seeds = np.zeros((len(rows), per), dtype=np.uint64)
        for k, r in enumerate(rows):
            if self.gumbel:
                gum[k] = self.rng.gumbel(size=width)
            seeds[k] = self.rng.integers(0, 2**63, size=per, dtype=np.uint64)
        root_lg = lgn[rows]
        main = np.asarray(mains)[rows]
        parts = [np.nonzero(main % self.lanes == g)[0] for g in range(self.lanes)]
        live = [(s, ks) for s, ks in zip(self.searches, parts) if len(ks)]
        for s, ks in live:
            s.begin(main[ks], root_lg[ks], softmax_rows(root_lg[ks]),
                    None if gum is None else gum[ks], seeds[ks])

        def drive(s):
            while True:
                k = s.run()
                if k == 0:
                    return
                lgl, v = self._eval(net, s.obs[:k], s.masks[:k], s.privs[:k], dev)
                s.feed(lgl, softmax_rows(lgl), v)

        ths = [threading.Thread(target=drive, args=(s,)) for s, _ in live[1:]]
        for t in ths:
            t.start()
        try:
            drive(live[0][0])
        finally:
            for t in ths:
                t.join()
        for s, ks in live:
            act, vis, ta, tp = s.results()
            # rows with fewer than TOPN legal moves: pad with the top move at p 0 (the Python
            # player pads with zero-probability moves; a -1 would index out of range in training)
            pad = ta < 0
            ta = np.where(pad, ta[:, :1], ta)
            tp = np.where(pad, 0, tp).astype(tp.dtype)
            r = rows[ks]
            choice[r] = act
            self.last_tgt_a[r], self.last_tgt_p[r] = ta, tp
            self.last_rows[r] = vis.astype(np.int64)
            self.stats["deviated"] += int((act != greedy[r]).sum())
        self.stats["searched"] += len(rows)
        self.stats["ms"] += 1000 * (time.perf_counter() - t0)
        return choice

    def after_step(self, env, mains):
        for s in self.searches or ():
            s.after_step(mains)

    def all_stats(self):
        st = dict(self.stats)
        for s in self.searches or ():
            for k, v in s.stats().items():
                st[f"rs_{k}"] = st.get(f"rs_{k}", 0) + v
        st["eval_s"] = round(self.eval_s, 1)
        return st


def player_of(a, gumbel, vnet=None, evaluator=None):
    return RustMCTSPlayer(a.sims, a.m, a.wave, a.c_puct, a.c_visit, a.c_scale, a.depth,
                          hp_coef=a.hp_coef, pot_w=a.pot_w, gumbel=gumbel, seed=a.seed,
                          world=a.world, vnet=vnet, threads=a.search_threads, lanes=a.lanes,
                          evaluator=evaluator)


def cmd_run(a):
    net, dev = C.net_and_dev(a.ckpt, a.weights)
    recs = C.read_jsonl(pathlib.Path(a.bench).expanduser())
    if a.only:
        recs = [r for r in recs if C.room_of(r) in a.only.split(",")]
    if a.limit:
        recs = recs[:a.limit]
    mis = M.FightMistakes(len(recs) * a.reps)
    player = player_of(a, not a.no_gumbel, M.load_vnet(a.value_ckpt, a.weights, dev))
    t0 = time.time()
    res = M.play(net, dev, recs, a.reps, 7_000_000, a.fights, player, a.threads,
                 on_step=mis.step)
    wall = time.time() - t0
    st = player.all_stats()
    res.update({f"mis_{k}": v for k, v in mis.c.items()})
    res["room"] = np.array([C.room_of(r) for r in recs for _ in range(a.reps)])
    res["act"] = np.array([r["act"] for r in recs for _ in range(a.reps)])
    res["enc"] = np.array([r["encounter"] for r in recs for _ in range(a.reps)])
    res["ids"] = np.array([r["id"] for r in recs for _ in range(a.reps)])
    dec = max(st.get("decisions", 1), 1)
    meta = dict(ckpt=str(a.ckpt), sims=a.sims, m=a.m, wave=a.wave, c_puct=a.c_puct, depth=a.depth,
                reps=a.reps, gumbel=not a.no_gumbel, world=a.world, pot_w=a.pot_w,
                hp_coef=a.hp_coef, value_ckpt=a.value_ckpt, wall_s=wall, stats=st, rust=True)
    np.savez(a.out, meta=json.dumps(meta),
             **{k: v for k, v in res.items() if isinstance(v, np.ndarray)})
    print(json.dumps(dict(fights=len(res["died"]), died=float(res["died"].mean()), wall_s=wall,
                          decisions=dec, searched=st.get("searched", 0),
                          searched_per_s=st.get("searched", 0) / wall,
                          batch_ms_per_decision=1000 * wall / dec, stats=st)), flush=True)
    return 0


def lean_starts(path, mix, count, rng, exclude=()):
    """`cb1_gumbel.starts` (the same draws, so the same records) without holding every record:
    one pass keeps each line's offset by room, a second parses only the chosen lines."""
    by = {room: [] for room in C.ROOMS}
    with open(path, "rb") as f:
        while True:
            at = f.tell()
            line = f.readline()
            if not line:
                break
            if not line.strip():
                continue
            r = json.loads(line)
            if r.get("id") not in exclude:
                by[C.room_of(r)].append(at)
    out = []
    for room, share in zip(("boss", "elite", "hallway"), mix):
        n = int(round(count * share))
        if n and by[room]:
            out += [by[room][i] for i in rng.choice(len(by[room]), n, replace=len(by[room]) < n)]
    order = rng.permutation(len(out))
    recs = []
    with open(path, "rb") as f:
        for i in order:
            f.seek(out[i])
            recs.append(json.loads(f.readline()))
    return recs


def cmd_gen(a):
    import cb1_gumbel as G

    net, dev = C.net_and_dev(a.ckpt, a.weights)
    rng = np.random.default_rng(a.seed)
    exclude = set()
    for b in a.exclude.split(","):
        if b:
            exclude |= {r["id"] for r in C.read_jsonl(pathlib.Path(b).expanduser())}
    recs = lean_starts(pathlib.Path(a.starts).expanduser(), [float(x) for x in a.mix.split(",")],
                       a.count, rng, exclude)
    player = player_of(a, gumbel=True, vnet=M.load_vnet(a.value_ckpt, a.weights, dev))
    rec = G.TargetRecorder(a.out, 0.0, a.chunk, a.seed)
    off = a.seed * 10_000_000
    t0 = time.time()
    res = M.play(net, dev, recs, 1, a.seed, a.fights, player, a.threads,
                 on_step=lambda env, idx, k_of, lg, act: rec.step(player, env, idx, k_of + off,
                                                                  lg, act),
                 on_end=lambda k, w, hp, mx: rec.end(k + off, w, hp, mx))
    rec.close(off)
    meta = dict(ckpt=str(a.ckpt), starts=a.starts, count=a.count, seed=a.seed, mix=a.mix,
                sims=a.sims, m=a.m, wave=a.wave, wall_s=time.time() - t0,
                stats=player.all_stats(), died=float(res["died"].mean()))
    (pathlib.Path(a.out) / f"meta{off:09d}.json").write_text(json.dumps(meta))
    print(json.dumps(meta), flush=True)
    return 0


class DetEval:
    """A deterministic evaluator for the identity check: each distinct row evaluated alone on
    the CPU in float32 (no batch effects), memoised; the same function for both players."""

    def __init__(self, net):
        import copy

        import torch

        torch.set_num_threads(4)
        self.net = copy.deepcopy(net).cpu().float().eval()
        self.memo = {}
        self.calls = 0

    def rows(self, obs, masks, priv):
        import rln36_boss as B
        import torch
        from sts2.e2e import encode as E

        f = B.flags(self.net)
        lg_out, v_out = [], []
        for i in range(len(obs)):
            key = obs[i].tobytes() + masks[i].tobytes() + priv[i].tobytes()
            hit = self.memo.get(key)
            if hit is None:
                self.calls += 1
                o = torch.from_numpy(obs[i:i + 1].copy())
                mk = torch.from_numpy(masks[i:i + 1].copy().view(np.int64))
                pv = torch.from_numpy(priv[i:i + 1].copy())
                with torch.no_grad():
                    tok = E.encode(o, mk, **f)
                    lg, h, valid = self.net.policy(tok)
                    v = self.net.value(h, valid, E.encode_priv(pv), tok["combat"])
                    lg = lg.float().nan_to_num(nan=float("-inf"))
                    legal = E.mask_bits(mk, discard=f["discard"])[:, : lg.shape[1]]
                    lg = lg.masked_fill(~legal, float("-inf"))
                hit = (lg[0].numpy().astype(np.float32), float(v.float()[0]))
                self.memo[key] = hit
            lg_out.append(hit[0])
            v_out.append(hit[1])
        return np.stack(lg_out).astype(np.float64), np.array(v_out, dtype=np.float64)


def cmd_identity(a):
    """Both players on the same fights with one deterministic evaluator; per fight, the moves
    until the first difference."""
    import torch

    net, dev = C.net_and_dev(a.ckpt, a.weights)
    det = DetEval(net)
    recs = C.read_jsonl(pathlib.Path(a.bench).expanduser())
    recs = [r for r in recs if C.room_of(r) in a.only.split(",")][:a.limit]

    def det_logits(net_, env, idx, dev_):
        idx = np.asarray(idx)
        z = np.zeros((len(idx), env.priv.shape[1]), dtype=np.uint8)  # policy rows: no priv
        lg, _ = det.rows(env.obs[idx], env.masks[idx], z)
        return torch.from_numpy(lg.astype(np.float32))

    def det_eval_net(net_, env, idx, dev_, value=True, vnet=None):
        idx = np.asarray(idx)
        return det.rows(env.obs[idx], env.masks[idx], env.priv[idx])

    C.logits = det_logits
    M.eval_net = det_eval_net
    kw = dict(sims=a.sims, m=a.m, wave=a.wave, pot_w=a.pot_w, gumbel=not a.no_gumbel,
              seed=a.seed, world=a.world)
    moves = {}
    for name in ("python", "rust"):
        if name == "python":
            player = M.MCTSPlayer(**kw)
        else:
            player = RustMCTSPlayer(**kw, threads=a.search_threads, lanes=a.lanes,
                                    evaluator=lambda n_, o, mk, pv, d: det.rows(o, mk, pv))
        seq = collections.defaultdict(list)

        def on_step(env, idx, k_of, lg, act, seq=seq):
            for k, x in zip(k_of, act):
                seq[int(k)].append(int(x))
        t0 = time.time()
        res = M.play(net, dev, recs, 1, 7_000_000, a.fights, player, a.threads, on_step=on_step)
        st = player.all_stats() if hasattr(player, "all_stats") else dict(player.stats)
        print(name, f"{time.time() - t0:.0f}s", "died", res["died"].tolist(), "evals", det.calls,
              json.dumps({k: v for k, v in st.items() if k != "ms"}), flush=True)
        moves[name] = seq
    same_fights, same_moves, total, first = 0, 0, 0, []
    for k in sorted(moves["python"]):
        p, r = moves["python"][k], moves["rust"].get(k, [])
        n = 0
        while n < min(len(p), len(r)) and p[n] == r[n]:
            n += 1
        same_fights += p == r
        same_moves += n
        total += len(p)
        if p != r:
            first.append((k, n, len(p), len(r)))
    out = dict(fights=len(moves["python"]), identical_fights=same_fights,
               moves_before_first_difference=same_moves, python_moves=total, differences=first)
    print(json.dumps(out), flush=True)
    pathlib.Path(a.out).write_text(json.dumps(out, indent=1))
    return 0 if same_fights == len(moves["python"]) else 1


def search_flags(p):
    p.add_argument("--weights", default="raw")
    p.add_argument("--sims", type=int, default=64)
    p.add_argument("--m", type=int, default=8)
    p.add_argument("--wave", type=int, default=16)
    p.add_argument("--c-puct", type=float, default=1.25)
    p.add_argument("--c-visit", type=float, default=50.0)
    p.add_argument("--c-scale", type=float, default=0.1)
    p.add_argument("--depth", type=int, default=40)
    p.add_argument("--no-gumbel", action="store_true")
    p.add_argument("--hp-coef", type=float, default=0.3)
    p.add_argument("--pot-w", type=float, default=0.10)
    p.add_argument("--world", default="paired", choices=tuple(WORLDS))
    p.add_argument("--value-ckpt", default="")
    p.add_argument("--seed", type=int, default=0)
    p.add_argument("--fights", type=int, default=1024)
    p.add_argument("--threads", type=int, default=4, help="the real envs' step threads")
    p.add_argument("--search-threads", type=int, default=12)
    p.add_argument("--lanes", type=int, default=2, help="Searches whose Rust work overlaps GPU eval")


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("run")
    r.add_argument("ckpt")
    r.add_argument("--bench", default=str(C.DATA / "bench.jsonl"))
    r.add_argument("--only", default="")
    r.add_argument("--limit", type=int, default=0)
    r.add_argument("--reps", type=int, default=1)
    r.add_argument("--out", required=True)
    search_flags(r)
    g = sub.add_parser("gen")
    g.add_argument("ckpt")
    g.add_argument("--starts", default="~/.cache/spirebot-data/cb1/train_b.jsonl")
    g.add_argument("--exclude", default=str(C.DATA / "bench.jsonl") + ","
                   "~/.cache/spirebot-data/cb1/panel_bench.jsonl")
    g.add_argument("--count", type=int, required=True)
    g.add_argument("--mix", default="0.5,0.4,0.1", help="boss, elite, hallway shares")
    g.add_argument("--chunk", type=int, default=100_000)
    g.add_argument("--out", required=True)
    search_flags(g)
    i = sub.add_parser("identity")
    i.add_argument("ckpt")
    i.add_argument("--bench", default="~/.cache/spirebot-data/cb1/panel_bench.jsonl")
    i.add_argument("--only", default="boss")
    i.add_argument("--limit", type=int, default=8)
    i.add_argument("--out", default="target/cb2/identity.json")
    search_flags(i)
    a = p.parse_args()
    return {"run": cmd_run, "gen": cmd_gen, "identity": cmd_identity}[a.cmd](a)


if __name__ == "__main__":
    raise SystemExit(main())
