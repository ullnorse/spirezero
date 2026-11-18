"""CB1 stage 2: Gumbel MuZero-style MCTS over honest determinized worlds, for play and for
expert iteration.

    python python/cb1_mcts.py run CKPT --sims 64 [--bench F.jsonl --only boss] --reps R --out F.npz
    python python/cb1_mcts.py gen CKPT --starts F.jsonl --count N --seed S --out DIR
    python python/cb1_mcts.py train ...                     # naz3_ei.py train on gen's data
    python python/cb1_mcts.py value CKPT --out F.json       # the value head as a leaf evaluator

**Tree.** One tree per real fight, over the player's decisions within and across turns. A node is
a visible position: its children are keyed by (action, hash of the observation after it), so a
draw, an enemy's roll or anything else hidden that the action reveals branches into chance
children, and a node's legal moves are the same in every world that reaches it. Edge statistics
(visits N, value sum W) are per action, pooled over its chance children.

**Worlds** (information-set MCTS, honest): every simulation starts from a fresh determinization
of the real fight, `Sts2RunEnvs.copy(reseed)`: the draw pile reshuffled and the fight's streams
reseeded, the visible state kept. Its moves are applied in that world; the world decides which
chance child follows.

**Simulation.** Descend from the root: at the root the action is assigned by sequential halving
(below); below it PUCT, `Q(a) + c_puct * P(a) * sqrt(N) / (1 + N(a))`, with an unvisited action's
Q completed by the node's mean value. A simulation stops at a new node (expanded with the net:
prior P = softmax of the masked logits, value V), at the fight's end, or after `depth` plies
(evaluated by V). Utility: a fight's end scores `won + hp_coef * HP/maxHP + pot_w * potions`; a
leaf `V + hp_coef * HP/maxHP + pot_w * potions` (V is the combat value head: the expected rest of
the gym return, P(win) + `hp_coef` x the HP fraction still to change, so both are on one scale).
V reads privileged tokens (the draw order, the enemies' coming moves); it is only ever evaluated
on a reseeded world, whose hidden state is a sample, never the real fight's (the policy reads
the real fight's visible observation only). Concurrent simulations of one tree use a virtual
loss (an edge's visit counts at selection, its value at backup).

**Root** (Gumbel MuZero): `m` actions drawn without replacement by Gumbel-top-k over the logits;
`ceil(log2 m)` phases of sequential halving split the budget `sims`; after each phase the better
half by `g + logits + sigma(q)` stays. sigma(q) = (c_visit + max N) * c_scale * q, q min-max
normalised over the root's completed Q. The move played is the survivor (`g` dropped with
`--no-gumbel`: the evaluation player). The improved policy `softmax(logits + sigma(completed Q))`
is the distillation target. The tree below the move played is kept for the next decision when
the real observation matches a child.

**Batching.** All live fights search together: a wave gives each fight up to `wave` simulations
(scratch envs), steps them in lockstep and evaluates all new leaves in one net batch.
"""

import argparse
import collections
import json
import math
import pathlib
import sys
import time

import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import naz3_combat as C  # noqa: E402

TOPN = 8


def phi(obs):
    hp = np.maximum(0, C.i16_col(obs, C.H_HP)).astype(np.float64)
    return hp / np.maximum(1, C.i16_col(obs, C.H_MAXHP)).astype(np.float64)


def eval_net(net, env, idx, dev, value=True, vnet=None):
    """Masked logits (numpy float64) and, with `value`, the combat value of env rows `idx`
    (the value reads the env's privileged buffer: call it on reseeded worlds only). With
    `vnet` the value comes from that net (its own trunk): a frozen evaluator beside a policy
    net trained by expert iteration (the value head reads the trunk, so it can't stay frozen
    inside a net whose trunk changes)."""
    import rln36_boss as B
    import torch
    from sts2.e2e import encode as E

    f = B.flags(net)
    lgs, vs = [], []
    for k in range(0, len(idx), C.GPU_CHUNK):
        rows = idx[k:k + C.GPU_CHUNK]
        obs = torch.from_numpy(env.obs[rows]).to(dev)
        masks = torch.from_numpy(env.masks.view(np.int64)[rows]).to(dev)
        with torch.no_grad():
            tok = E.encode(obs, masks, **f)
            with torch.autocast(dev.type, dtype=torch.bfloat16, enabled=dev.type == "cuda"):
                lg, h, valid = net.policy(tok)
                if value:
                    ptok = E.encode_priv(torch.from_numpy(env.priv[rows]).to(dev))
                    if vnet is not None:
                        _, h, valid = vnet.policy(tok)
                        v = vnet.value(h, valid, ptok, tok["combat"])
                    else:
                        v = net.value(h, valid, ptok, tok["combat"])
            lg = lg.float().nan_to_num(nan=float("-inf"))
            legal = E.mask_bits(masks, discard=f["discard"])[:, : lg.shape[1]]
            lgs.append(lg.masked_fill(~legal, float("-inf")).cpu().numpy().astype(np.float64))
            if value:
                vs.append(v.float().cpu().numpy().astype(np.float64))
    lg = np.concatenate(lgs)
    return lg, (np.concatenate(vs) if value else None)


def key_of(env, i):
    """A visible position: the observation and the legal-action mask (the mask can differ
    between positions whose observations are equal)."""
    return hash(env.obs[i].tobytes() + env.masks[i].tobytes())


def turn_of(obs):
    import rln19_act2 as R

    at = R.COMBAT_AT + R.C_TURN
    return obs[:, at].astype(np.int32) | obs[:, at + 1].astype(np.int32) << 8


def softmax(z):
    z = np.where(np.isfinite(z), z, -np.inf)
    e = np.exp(z - z.max())
    return e / e.sum()


class Node:
    __slots__ = ("prior", "legal", "logits", "N", "W", "children", "v", "vsum", "vn")

    def __init__(self, logits, v):
        self.logits = logits
        self.legal = np.isfinite(logits)
        self.prior = softmax(logits)
        self.N = np.zeros(len(logits))
        self.W = np.zeros(len(logits))
        self.children = {}
        self.v = v  # this node's own leaf utility (None for a root never evaluated as a leaf)
        self.vsum, self.vn = 0.0, 0

    def mean(self):
        n = self.N.sum()
        if n > 0:
            return self.W.sum() / n
        return self.v if self.v is not None else 0.0


class MCTSPlayer:
    """See the module docs. The `choose` interface of `naz3_combat.play` players."""

    def __init__(self, sims=64, m=16, wave=16, c_puct=1.25, c_visit=50.0, c_scale=0.1, depth=40,
                 hp_coef=0.3, pot_w=0.03, gumbel=True, seed=0, rooms=C.ROOMS, keep_tree=True,
                 leaf="value", world="fresh", vnet=None):
        self.sims, self.m, self.wave, self.c_puct = sims, m, wave, c_puct
        self.c_visit, self.c_scale, self.depth = c_visit, c_scale, depth
        self.hp_coef, self.pot_w, self.gumbel, self.keep_tree = hp_coef, pot_w, gumbel, keep_tree
        self.rooms = set(rooms)
        self.leaf = leaf  # "value", "turn" (greedy to the turn's end, then V), "full" (to the end)
        # worlds: "fresh" (a new reseed per simulation), "paired" (per decision, simulation j of
        # every root action uses the same reseed j: common random numbers), "one" (one reseed per
        # decision), "exact" (the real hidden state: dishonest, a diagnostic upper bound only)
        self.world = world
        self.vnet = vnet
        self.wseed, self.wcnt = {}, collections.Counter()
        self.rng = np.random.default_rng(seed)
        self.stats = collections.Counter()
        self.base = 0
        self.env_kw = dict(priv=True)
        self.trees = {}  # main env -> (root node, hash of its obs)
        self.pending = {}  # main env -> (root node, action played), until the real step
        self.last_tgt_a = self.last_tgt_p = None

    def scratch(self, fights):
        return fights * self.wave

    # --- utilities -------------------------------------------------------------------------

    def leaf_util(self, obs, v):
        return v + self.hp_coef * phi(obs) + self.pot_w * C.belt_counts(obs)

    def end_util(self, env, d):
        from sts2.env import RUN_ENDS

        obs = env.obs[d].copy()
        done = env.done[d].astype(bool)
        obs[done] = env.final_obs[d[done]]
        ends = np.array([RUN_ENDS[e] for e in env.end[d]])
        won = ~done | np.isin(ends, ("gym_end", "victory"))
        return np.where(won, 1.0 + self.hp_coef * phi(obs) + self.pot_w * C.belt_counts(obs), 0.0)

    def sigma(self, q, maxn):
        return (self.c_visit + maxn) * self.c_scale * q

    def completed_q(self, node):
        """Per-action Q, the unvisited ones completed by the prior-weighted mean of the visited
        (Gumbel MuZero's mixed value), min-max normalised over the legal actions."""
        vis = node.N > 0
        q = np.where(vis, node.W / np.maximum(node.N, 1), 0.0)
        if vis.any():
            pv = node.prior[vis]
            mix = float((pv * q[vis]).sum() / max(pv.sum(), 1e-12))
        else:
            mix = node.mean()
        q = np.where(vis, q, mix)
        lo, hi = q[node.legal].min(), q[node.legal].max()
        return np.where(node.legal, (q - lo) / max(hi - lo, 1e-6), 0.0)

    def puct(self, node):
        n = node.N.sum()
        q = np.where(node.N > 0, node.W / np.maximum(node.N, 1), node.mean())
        s = q + self.c_puct * node.prior * math.sqrt(n + 1) / (1 + node.N)
        return int(np.argmax(np.where(node.legal, s, -np.inf)))

    # --- search ----------------------------------------------------------------------------

    def choose(self, net, env, dev, mains, lg, room_ok):
        from sts2 import HOLD

        t0 = time.perf_counter()
        lgn = lg.float().cpu().numpy().astype(np.float64)
        n, width = lgn.shape
        greedy = lgn.argmax(1)
        dead = ~np.isfinite(lgn).any(1)  # an empty slot between fights (no decision yet)
        choice = greedy.copy()
        # HOLD: any step (the search's too) starts the slot's next fight, so a move chosen now
        # could meet a different position at the real step
        choice[dead] = HOLD
        self.stats["empty_rows"] += int(dead.sum())
        safe = np.where(dead[:, None], 0.0, lgn)
        P = np.exp(safe - safe.max(1, keepdims=True))
        P /= P.sum(1, keepdims=True)
        top = np.argsort(-P, 1)[:, :TOPN]
        self.last_tgt_a = top.astype(np.int16)
        self.last_tgt_p = np.take_along_axis(P, top, 1).astype(np.float32)
        nlegal = np.isfinite(lgn).sum(1)
        search = room_ok & (nlegal > 1) & ~dead
        if dead.any():
            # a slot whose new fight ended at its reset ends at the next engine step; that step
            # must be the real one (`naz3_combat.play` hands out the next record on it), so no
            # search runs this step
            self.stats["skipped_steps"] += 1
            search[:] = False
        self.last_searched = search.astype(np.int32)
        self.last_rows = np.zeros(n, dtype=np.int64)
        self.stats["decisions"] += n
        if not search.any():
            return choice
        rows = np.nonzero(search)[0]
        self.stats["searched"] += len(rows)
        roots, gum, cands = {}, {}, {}
        self.wcnt.clear()
        for r in rows:
            f = int(mains[r])
            h = key_of(env, f)
            old = self.trees.get(f)
            node = old[0] if (old is not None and old[1] == h and self.keep_tree) else None
            if node is None or node.logits.shape[0] != width:
                node = Node(lgn[r], None)
            else:
                self.stats["reused"] += 1
                node.logits, node.prior = lgn[r], softmax(lgn[r])
                node.legal = np.isfinite(lgn[r])
            roots[f] = node
            g = self.rng.gumbel(size=width) if self.gumbel else np.zeros(width)
            gum[f] = g
            k = min(self.m, int(nlegal[r]))
            self.wseed[f] = self.rng.integers(0, 2**63, size=1 if self.world == "one" else self.sims,
                                              dtype=np.uint64)
            # stable: equal scores keep the action order (CB2: the Rust search's order)
            cands[f] = list(np.argsort(-np.where(node.legal, g + node.logits, -np.inf),
                                       kind="stable")[:k])
        phases = {f: max(1, math.ceil(math.log2(len(c)))) if len(c) > 1 else 1
                  for f, c in cands.items()}
        used = {f: 0 for f in roots}
        F = self.base
        for ph in range(max(phases.values())):
            # this phase's simulations per fight: its budget share, round-robin over survivors
            todo = {}
            for f, c in cands.items():
                if ph >= phases[f] or len(c) <= 1:
                    continue
                share = max(len(c), (self.sims - used[f]) // (phases[f] - ph))
                per = max(1, share // len(c))
                todo[f] = [a for a in c for _ in range(per)]
                used[f] += per * len(c)
            while any(todo.values()):
                batch = {}
                for f in todo:
                    if todo[f]:
                        batch[f], todo[f] = todo[f][:self.wave], todo[f][self.wave:]
                self.simulate(net, env, dev, roots, batch, F, HOLD)
            for f, c in cands.items():
                if ph >= phases[f] or len(c) <= 1:
                    continue
                node = roots[f]
                sq = self.sigma(self.completed_q(node), node.N.max())
                c.sort(key=lambda a: -(gum[f][a] + node.logits[a] + sq[a]))
                cands[f] = c[:max(1, len(c) // 2)]
        for r in rows:
            f = int(mains[r])
            node = roots[f]
            sq = self.sigma(self.completed_q(node), node.N.max())
            c = cands[f]
            pick = int(c[0]) if len(c) == 1 else int(max(c, key=lambda a: gum[f][a] + node.logits[a] + sq[a]))
            if not np.isfinite(lgn[r, pick]):  # never expected: keys include the mask
                self.stats["illegal_pick"] += 1
                pick = int(greedy[r])
            choice[r] = pick
            t = softmax(np.where(node.legal, node.logits + sq, -np.inf))
            o = np.argsort(-t)[:TOPN]
            self.last_tgt_a[r], self.last_tgt_p[r] = o, t[o]
            self.last_rows[r] = int(node.N.sum())
            if pick != greedy[r]:
                self.stats["deviated"] += 1
            self.pending[f] = (node, pick)  # re-rooted once the real obs is known
        self.stats["ms"] += 1000 * (time.perf_counter() - t0)
        return choice

    def after_step(self, env, mains):
        """Called after the real env stepped: re-root each tree at the child the real
        observation chose (or drop it)."""
        for f in mains:
            f = int(f)
            node, a = self.pending.pop(f, (None, None))
            self.trees.pop(f, None)
            if node is None or env.done[f]:
                continue
            h = key_of(env, f)
            child = node.children.get((a, h))
            if child is not None and child.N.sum() > 0:
                self.trees[f] = (child, h)

    def rollout(self, net, env, dev, d, lg, u, HOLD):
        """Leaf utilities of envs `d` (at a new node: logits `lg`, value utility `u`) by playing
        on greedily: to the end of the current turn ("turn", then V) or of the fight ("full").
        The node itself keeps its prior; only the utility backed up changes."""
        u = u.copy()
        turn0 = turn_of(env.obs[d])
        live = np.arange(len(d))
        ok = np.isfinite(lg).any(1)
        live = live[ok]
        a = lg[ok].argmax(1)
        steps = 0
        while len(live) and steps < C.ROLLOUT_CAP:
            acts = np.full(env.n, HOLD, dtype=np.uint16)
            acts[d[live]] = a
            env.step(acts)
            steps += 1
            self.stats["rollout_steps"] += len(live)
            e = d[live]
            fin = env.done[e].astype(bool) | (env.obs[e, C.H_PHASE] != C.PHASE_COMBAT)
            if fin.any():
                u[live[fin]] = self.end_util(env, e[fin])
            stop = np.zeros(len(live), dtype=bool)
            if self.leaf == "turn":
                stop = ~fin & (turn_of(env.obs[e]) != turn0[live])
            if stop.any():
                _, v = eval_net(net, env, e[stop], dev, vnet=self.vnet)
                u[live[stop]] = self.leaf_util(env.obs[e[stop]], v)
            live = live[~fin & ~stop]
            if len(live):
                lgl, v = eval_net(net, env, d[live], dev, vnet=self.vnet)
                dead = ~np.isfinite(lgl).any(1)  # no move the net can score: stop with V
                if dead.any():
                    u[live[dead]] = self.leaf_util(env.obs[d[live[dead]]], v[dead])
                    live, lgl = live[~dead], lgl[~dead]
                a = lgl.argmax(1)
        return u

    def simulate(self, net, env, dev, roots, batch, F, HOLD):
        """One wave: simulations `batch[f]` (root actions) for fights f."""
        src, dst, path, first = [], [], [], []
        for f, acts in batch.items():
            for j, a in enumerate(acts):
                src.append(f)
                dst.append(F + f * self.wave + j)
                path.append([(roots[f], int(a))])
                roots[f].N[a] += 1  # virtual loss: the visit now, the value at backup
        src, dst = np.asarray(src), np.asarray(dst)
        if self.world == "exact":
            env.copy(src, dst)
        elif self.world == "fresh":
            env.copy(src, dst, reseed=self.rng.integers(0, 2**63, size=len(src), dtype=np.uint64))
        else:
            seeds = []
            for f, acts in batch.items():
                for a in acts:
                    s = self.wseed[f]
                    seeds.append(s[self.wcnt[(f, int(a))] % len(s)])
                    self.wcnt[(f, int(a))] += 1
            env.copy(src, dst, reseed=np.asarray(seeds, dtype=np.uint64))
        bits = np.unpackbits(np.ascontiguousarray(env.masks[dst]).view(np.uint8), axis=1,
                             bitorder="little")
        ok = bits[np.arange(len(dst)), [p[0][1] for p in path]].astype(bool)
        if not ok.all():  # the world's mask differs from the real fight's: drop the simulation
            self.stats["dropped_sims"] += int((~ok).sum())
            for i in np.nonzero(~ok)[0]:
                node, a = path[i][0]
                node.N[a] -= 1
                path[i] = []
        live = np.nonzero(ok)[0]
        util = np.zeros(len(dst))
        leaf_node = [None] * len(dst)
        depth = 0
        while len(live):
            want = np.array([path[i][-1][1] for i in live])
            bits = np.unpackbits(np.ascontiguousarray(env.masks[dst[live]]).view(np.uint8),
                                 axis=1, bitorder="little")
            bad = ~bits[np.arange(len(live)), want].astype(bool)
            if bad.any():  # a move the world's mask forbids: end the simulation there (debug)
                for k in np.nonzero(bad)[0][:2]:
                    if self.stats["illegal_sim"] < 4:
                        node, a = path[live[k]][-1]
                        print("illegal sim move", int(a), "depth", len(path[live[k]]),
                              "node legal", np.nonzero(node.legal)[0][:12].tolist(),
                              "mask legal", np.nonzero(bits[k])[0][:12].tolist(),
                              "prior nan", bool(np.isnan(node.prior).any()), flush=True)
                    self.stats["illegal_sim"] += 1
                for i in live[bad]:
                    node, a = path[i].pop()
                    node.N[a] -= 1
                    if path[i]:  # value of the position reached: its node's own estimate
                        util[i] = node.v if node.v is not None else node.mean()
                live, want = live[~bad], want[~bad]
                if not len(live):
                    break
            acts = np.full(env.n, HOLD, dtype=np.uint16)
            acts[dst[live]] = want
            env.step(acts)
            self.stats["sim_steps"] += len(live)
            depth += 1
            d = dst[live]
            fin = env.done[d].astype(bool) | (env.obs[d, C.H_PHASE] != C.PHASE_COMBAT)
            if fin.any():
                util[live[fin]] = self.end_util(env, d[fin])
                self.stats["sim_ends"] += int(fin.sum())
            nxt, need = [], []
            for i in live[~fin]:
                node, a = path[i][-1]
                h = key_of(env, dst[i])
                child = node.children.get((a, h))
                if child is None or depth >= self.depth or not child.legal.any():
                    if child is not None and not child.legal.any():
                        self.stats["dead_end"] += 1  # no move the net can score: a leaf
                    need.append((i, h))
                    continue
                b = self.puct(child)
                child.N[b] += 1
                path[i].append((child, b))
                nxt.append(i)
            if need:
                ii = np.array([i for i, _ in need])
                lg, v = eval_net(net, env, dst[ii], dev, vnet=self.vnet)
                u = self.leaf_util(env.obs[dst[ii]], v)
                if self.leaf != "value":
                    u = self.rollout(net, env, dev, dst[ii], lg, u, HOLD)
                self.stats["leaf_evals"] += len(ii)
                for k, (i, h) in enumerate(need):
                    node, a = path[i][-1]
                    if (a, h) not in node.children:
                        node.children[(a, h)] = Node(lg[k], float(u[k]))
                    util[i] = u[k]
            live = np.array(nxt, dtype=np.int64)
        for i, p in enumerate(path):
            for node, a in p:
                node.W[a] += util[i]


def play(net, dev, recs, reps, seed, fights, player, threads, on_step=None, on_end=None):
    """`naz3_combat.play` with the tree re-rooted after every real step."""
    def step_hook(env, idx, k_of, lg, a):
        if on_step is not None:
            on_step(env, idx, k_of, lg, a)
        player._pending = idx

    import rln36_boss as B

    orig = B.make_env

    def make_env(*a, **kw):
        env = orig(*a, **kw)
        step = env.step

        def stepped(acts):
            out = step(acts)
            if getattr(player, "_pending", None) is not None:
                player.after_step(env, player._pending)
                player._pending = None
            return out
        env.step = stepped
        return env
    B.make_env = make_env
    try:
        return C.play(net, dev, recs, reps, seed, fights, player, threads, on_step=step_hook,
                      on_end=on_end)
    finally:
        B.make_env = orig


class FightMistakes:
    """`cb1_gumbel.Mistakes`, counted per fight (rollout k): turn ends, ends with energy left and
    a card playable (`unused`), of those below the visible incoming damage (`underblock`), ends
    with Osty at <= 3 HP against incoming damage (`osty_low`), potions and potions drunk while
    the enemies have >= 60% of their HP (`pot_early`)."""

    KEYS = ("ends", "unused", "underblock", "osty_low", "potions", "pot_early")

    def __init__(self, total):
        self.c = {k: np.zeros(total, dtype=np.int32) for k in self.KEYS}

    def step(self, env, idx, k_of, lg, a):
        import cb1_gumbel as G
        import rln36_boss as B

        kind = G.kinds(lg.shape[1])
        legal = G.torch_legal(lg)
        ka = kind[np.asarray(a)]
        k_of = np.asarray(k_of)
        end = ka == B.KIND_END
        pot = ka == B.KIND_POTION
        if end.any():
            e, blk, inc, osty = G.row_state(env.obs[idx[end]]).T
            card = (legal[end] & (kind == B.KIND_HAND)[None]).any(1)
            k = k_of[end]
            np.add.at(self.c["ends"], k, 1)
            np.add.at(self.c["unused"], k, ((e > 0) & card).astype(np.int32))
            np.add.at(self.c["underblock"], k, ((e > 0) & card & (blk < inc)).astype(np.int32))
            np.add.at(self.c["osty_low"], k, ((osty > 0) & (osty <= 3) & (inc > 0)).astype(np.int32))
        if pot.any():
            k = k_of[pot]
            np.add.at(self.c["potions"], k, 1)
            np.add.at(self.c["pot_early"], k, (C.enemy_dealt(env.obs[idx[pot]]) < 0.4).astype(np.int32))


def load_vnet(path, weights, dev):
    if not path:
        return None
    vnet, _ = C.net_and_dev(path, weights)
    vnet.eval()
    return vnet


def cmd_run(a):
    net, dev = C.net_and_dev(a.ckpt, a.weights)
    recs = C.read_jsonl(pathlib.Path(a.bench).expanduser())
    if a.only:
        recs = [r for r in recs if C.room_of(r) in a.only.split(",")]
    if a.limit:
        recs = recs[:a.limit]
    mis = FightMistakes(len(recs) * a.reps)
    t0 = time.time()
    if a.greedy or a.no_potions:  # the net alone, same seeds
        player = GreedyPlayer(True) if a.no_potions else None
        res = C.play(net, dev, recs, a.reps, 7_000_000, a.fights, player, a.threads,
                     on_step=mis.step)
    else:
        player = player_of(a, not a.no_gumbel, load_vnet(a.value_ckpt, a.weights, dev))
        res = play(net, dev, recs, a.reps, 7_000_000, a.fights, player, a.threads,
                   on_step=mis.step)
    wall = time.time() - t0
    st = dict(player.stats) if player else {}
    st.setdefault("decisions", 1)
    res.update({f"mis_{k}": v for k, v in mis.c.items()})
    res["room"] = np.array([C.room_of(r) for r in recs for _ in range(a.reps)])
    res["act"] = np.array([r["act"] for r in recs for _ in range(a.reps)])
    res["enc"] = np.array([r["encounter"] for r in recs for _ in range(a.reps)])
    res["ids"] = np.array([r["id"] for r in recs for _ in range(a.reps)])
    dec = max(st.get("decisions", 1), 1)
    meta = dict(ckpt=str(a.ckpt), sims=a.sims, m=a.m, wave=a.wave, c_puct=a.c_puct, leaf=a.leaf,
                depth=a.depth, reps=a.reps, gumbel=not a.no_gumbel, world=a.world,
                value_ckpt=a.value_ckpt, greedy=a.greedy or a.no_potions, wall_s=wall, stats=st)
    np.savez(a.out, meta=json.dumps(meta),
             **{k: v for k, v in res.items() if isinstance(v, np.ndarray)})
    print(json.dumps(dict(fights=len(res["died"]), died=float(res["died"].mean()), wall_s=wall,
                          decisions=dec, searched=st.get("searched", 0),
                          batch_ms_per_decision=1000 * wall / dec,
                          search_ms_per_call=st.get("ms", 0) / max(1, st.get("searched", 1)),
                          stats=st)), flush=True)
    return 0


class GreedyPlayer:
    """The net alone, greedy; with `no_potions` it never drinks a potion (a potion's worth: the
    same starts played with and without them)."""

    def __init__(self, no_potions=False):
        self.no_potions = no_potions
        self.env_kw = {}
        self.rooms = set(C.ROOMS)
        self.stats = collections.Counter()
        self.base = 0

    def scratch(self, fights):
        return 0

    def choose(self, net, env, dev, mains, lg, room_ok):
        import cb1_gumbel as G
        import rln36_boss as B

        x = lg.float().cpu().numpy()
        if self.no_potions:
            pot = G.kinds(x.shape[1]) == B.KIND_POTION
            y = x.copy()
            y[:, pot] = -np.inf
            ok = np.isfinite(y).any(1)
            x[ok] = y[ok]
        self.last_searched = np.zeros(len(mains), dtype=np.int32)
        self.last_rows = np.zeros(len(mains), dtype=np.int64)
        return x.argmax(1)


class ValueProbe:
    """Greedy player that, at every decision, evaluates the leaf utility of a reseeded copy of
    the fight (what the search sees at a leaf), to compare with how the fight ended."""

    def __init__(self, seed=0, hp_coef=0.3, pot_w=0.03):
        self.rng = np.random.default_rng(seed)
        self.env_kw = dict(priv=True)
        self.rooms = set(C.ROOMS)
        self.stats = collections.Counter()
        self.base = 0
        self.leaf = MCTSPlayer(hp_coef=hp_coef, pot_w=pot_w)
        self.rows = []  # (rollout k, turn-free decision index, predicted utility)

    def scratch(self, fights):
        return fights

    def choose(self, net, env, dev, mains, lg, room_ok):
        mains = np.asarray(mains)
        dst = self.base + mains
        env.copy(mains, dst, reseed=self.rng.integers(0, 2**63, size=len(mains), dtype=np.uint64))
        _, v = eval_net(net, env, dst, dev, vnet=getattr(self, "vnet", None))
        self.last_u = self.leaf.leaf_util(env.obs[dst], v)
        self.last_searched = np.zeros(len(mains), dtype=np.int32)
        self.last_rows = np.zeros(len(mains), dtype=np.int64)
        return lg.argmax(-1).cpu().numpy()


def cmd_value(a):
    net, dev = C.net_and_dev(a.ckpt, a.weights)
    recs = C.read_jsonl(pathlib.Path(a.bench).expanduser())
    if a.only:
        recs = [r for r in recs if C.room_of(r) in a.only.split(",")]
    if a.limit:
        recs = recs[:a.limit]
    probe = ValueProbe(a.seed)
    probe.vnet = load_vnet(a.value_ckpt, a.weights, dev)
    pred = collections.defaultdict(list)
    ends = {}

    def on_step(env, idx, k_of, lg, act):
        for k, u in zip(k_of, probe.last_u):
            pred[int(k)].append(float(u))

    def on_end(k, won, hp, mx):
        ends[int(k)] = (1.0 + 0.3 * hp / max(mx, 1)) if won else 0.0

    C.play(net, dev, recs, a.reps, 7_000_000, a.fights, probe, a.threads, on_step=on_step,
           on_end=on_end)
    ks = sorted(set(pred) & set(ends))
    out = {}
    groups = [("first decision", lambda p: p[:1], ks), ("all decisions", lambda p: p, ks)]
    for room in sorted({C.room_of(r) for r in recs}):
        for act in range(3):
            sub = [k for k in ks if C.room_of(recs[k // a.reps]) == room
                   and recs[k // a.reps]["act"] == act]
            if sub:
                groups.append((f"{room} act {act + 1}, all decisions", lambda p: p, sub))
    for name, sel, kk in groups:
        x = np.array([u for k in kk for u in sel(pred[k])])
        y = np.array([ends[k] for k in kk for _ in sel(pred[k])])
        bins = {}
        for lo in np.arange(0, 1.3, 0.2):  # calibration: mean outcome per predicted-utility bin
            b = (x >= lo) & (x < lo + 0.2)
            if b.sum() >= 20:
                bins[f"{lo:.1f}"] = [round(float(x[b].mean()), 3), round(float(y[b].mean()), 3),
                                     int(b.sum())]
        won = y > 0
        auc = float("nan")
        if won.any() and (~won).any():
            rank = np.argsort(np.argsort(x, kind="stable"), kind="stable") + 1.0  # ties: rare
            auc = float((rank[won].sum() - won.sum() * (won.sum() + 1) / 2)
                        / (won.sum() * (~won).sum()))
        out[name] = dict(n=int(len(x)), mae=float(np.abs(x - y).mean()),
                         corr=float(np.corrcoef(x, y)[0, 1]), win_auc=auc,
                         mean_pred=float(x.mean()), mean_actual=float(y.mean()),
                         fight_win=float(np.mean([ends[k] > 0 for k in kk])), fights=len(kk),
                         bins_pred_actual_n=bins)
    out["fights"] = len(ks)
    pathlib.Path(a.out).write_text(json.dumps(out, indent=1))
    print(json.dumps(out), flush=True)
    return 0


def cmd_gen(a):
    import cb1_gumbel as G

    net, dev = C.net_and_dev(a.ckpt, a.weights)
    rng = np.random.default_rng(a.seed)
    bench = {r["id"] for r in C.read_jsonl(C.DATA / "bench.jsonl")}
    recs = G.starts(pathlib.Path(a.starts).expanduser(), [float(x) for x in a.mix.split(",")],
                    a.count, rng, bench)
    player = player_of(a, gumbel=True, vnet=load_vnet(a.value_ckpt, a.weights, dev))
    rec = G.TargetRecorder(a.out, 0.0, a.chunk, a.seed)  # searched rows only
    off = a.seed * 10_000_000
    t0 = time.time()
    res = play(net, dev, recs, 1, a.seed, a.fights, player, a.threads,
               on_step=lambda env, idx, k_of, lg, act: rec.step(player, env, idx, k_of + off, lg, act),
               on_end=lambda k, w, hp, mx: rec.end(k + off, w, hp, mx))
    rec.close(off)
    meta = dict(ckpt=str(a.ckpt), starts=a.starts, count=a.count, seed=a.seed, mix=a.mix,
                sims=a.sims, m=a.m, wave=a.wave, wall_s=time.time() - t0,
                stats=dict(player.stats), died=float(res["died"].mean()))
    (pathlib.Path(a.out) / f"meta{off:09d}.json").write_text(json.dumps(meta))
    print(json.dumps(meta), flush=True)
    return 0


def player_of(a, gumbel, vnet=None):
    return MCTSPlayer(a.sims, a.m, a.wave, a.c_puct, a.c_visit, a.c_scale, a.depth,
                      gumbel=gumbel, seed=a.seed, leaf=a.leaf, world=a.world, vnet=vnet,
                      pot_w=a.pot_w)


def search_flags(p):
    p.add_argument("--weights", default="raw")
    p.add_argument("--sims", type=int, default=64)
    p.add_argument("--m", type=int, default=16)
    p.add_argument("--wave", type=int, default=16)
    p.add_argument("--c-puct", type=float, default=1.25)
    p.add_argument("--c-visit", type=float, default=50.0)
    p.add_argument("--c-scale", type=float, default=0.1)
    p.add_argument("--depth", type=int, default=40)
    p.add_argument("--no-gumbel", action="store_true")
    p.add_argument("--leaf", default="value", choices=("value", "turn", "full"))
    p.add_argument("--pot-w", type=float, default=0.03,
                   help="utility per potion kept (leaves and fight ends); 0.10 = a drunk potion's "
                        "measured worth on the panel")
    p.add_argument("--world", default="fresh", choices=("fresh", "paired", "one", "exact"))
    p.add_argument("--value-ckpt", default="", help="a frozen net for the leaf values")
    p.add_argument("--seed", type=int, default=0)
    p.add_argument("--fights", type=int, default=128)
    p.add_argument("--threads", type=int, default=8)


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("run")
    r.add_argument("ckpt")
    r.add_argument("--bench", default=str(C.DATA / "bench.jsonl"))
    r.add_argument("--only", default="")
    r.add_argument("--limit", type=int, default=0)
    r.add_argument("--reps", type=int, default=2)
    r.add_argument("--out", required=True)
    r.add_argument("--greedy", action="store_true", help="the net alone (no search)")
    r.add_argument("--no-potions", action="store_true", help="the net alone, never drinking")
    search_flags(r)
    g = sub.add_parser("gen")
    g.add_argument("ckpt")
    g.add_argument("--starts", default="~/.cache/spirebot-data/cb1/train_h.jsonl")
    g.add_argument("--count", type=int, required=True)
    g.add_argument("--mix", default="0.5,0.35,0.15", help="boss, elite, hallway shares")
    g.add_argument("--chunk", type=int, default=100_000)
    g.add_argument("--out", required=True)
    search_flags(g)
    v = sub.add_parser("value")
    v.add_argument("ckpt")
    v.add_argument("--bench", default=str(C.DATA / "bench.jsonl"))
    v.add_argument("--only", default="boss")
    v.add_argument("--reps", type=int, default=1)
    v.add_argument("--limit", type=int, default=0)
    v.add_argument("--out", required=True)
    search_flags(v)
    if len(sys.argv) > 1 and sys.argv[1] == "train":  # naz3_ei's trainer, unchanged
        import naz3_ei as EI

        sys.argv[0] = "naz3_ei.py"
        return EI.main()
    a = p.parse_args()
    return {"run": cmd_run, "gen": cmd_gen, "value": cmd_value}[a.cmd](a)


if __name__ == "__main__":
    raise SystemExit(main())
