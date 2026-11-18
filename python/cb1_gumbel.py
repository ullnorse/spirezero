"""CB1 stage 2: search in the loop. Gumbel policy improvement on determinized worlds, distilled
back into the combat net (expert iteration).

    python python/cb1_gumbel.py gen CKPT --starts F.jsonl --count N --seed S --out DIR
    python python/cb1_gumbel.py train CKPT --data DIR [DIR ...] --out CKPT2      # naz3_ei train
    python python/cb1_gumbel.py mistakes CKPT [--search] --out F.json          # on the NAZ3 bench

**Search** (`GumbelPlayer`, at a combat decision whose top action has probability < `pskip`, or
that is a targeted decision, below): candidates are the greedy move plus `k - 1` moves drawn
without replacement by Gumbel-top-k over the net's logits (Gumbel MuZero's root sampling), plus
forced alternatives at the decisions where the traces showed the net's mistakes:
- the greedy move ends the turn while energy is left and a card is playable: the best card play;
- the greedy move drinks a potion: the best move that does not.
Every candidate is applied in `m` reseeded copies of the fight (`Sts2RunEnvs.copy(reseed)`: the
draw pile and the fight's streams resampled, the visible state kept; honest), the same `m` seeds
for every candidate (common random numbers), and each copy is played to the fight's end by the
net, greedily. Utility of a copy: a win scores `1 + HP left / max HP + 0.05 x potions left`
(potions kept are worth a little, so a needless potion loses to the same line without it), a loss
`loss_w x` the enemies' HP fraction removed. Each candidate's mean is its paired gain over the
greedy move (same seeds) shrunk toward 0 by `z` standard errors, so a near-tie is a tie.

**Improved policy** (Gumbel MuZero's completed-Q target): `softmax(logits + beta * cq)` over the
legal moves, where `cq` is a candidate's shrunk Q and, for a move not searched, the prior-weighted
mean Q of the candidates. The move played in `gen` is `argmax(g + logits + beta * q)` over the
candidates (with the same Gumbel noise `g` that drew them: a sample of the improved policy);
`--no-gumbel` (evaluation) drops the noise.

**gen**: plays starts of a CB1 gym file (by room with `--mix`) once each with the search player
and stores rows in NAZ3's expert-iteration format (`naz3_ei.load_data`): every searched row with
the improved policy's top 8 as its target, `--keep` of the others with the net's own top 8 (an
anchor). **train** is `naz3_ei.py train` on that data (cross-entropy to the targets plus a fight
value head). **mistakes**: per decision rates of the targeted mistakes on the NAZ3 bench (see
`Mistakes`), for the net alone or with the search.
"""

import argparse
import collections
import json
import pathlib
import sys
import time

import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import naz3_combat as C  # noqa: E402
import naz3_ei as EI  # noqa: E402

TOPN = EI.TOPN


def row_state(obs):
    """Energy, block, visible incoming damage and Osty's HP (0: no Osty) of combat obs rows."""
    import rln19_act2 as R

    out = np.zeros((len(obs), 4), dtype=np.int32)
    for i, row in enumerate(obs):
        f = dict(zip(R.ROW_FIELDS, R.combat_row(row)))
        out[i] = f["energy"], f["block"], f["incoming"], f["osty_hp"] if f["osty"] else 0
    return out


def kinds(width):
    import rln36_boss as B

    return B.table()[:width, 0]


class GumbelPlayer(C.SearchPlayer):
    """See the module docs. `choose` also leaves `last_tgt_a` / `last_tgt_p` (the improved
    policy's top 8, or the net's for an unsearched row) for the recorder."""

    def __init__(self, k=4, m=8, beta=10.0, z=1.0, pskip=0.95, gumbel=True, force=True,
                 loss_w=0.5, seed=0, rooms=C.ROOMS):
        super().__init__(k=k, m=m, pskip=pskip, loss_w=loss_w, rooms=rooms, seed=seed)
        self.kmax = k + 2
        self.beta, self.z, self.gumbel, self.force = beta, z, gumbel, force

    def scratch(self, fights):
        return fights * self.kmax * self.m

    def choose(self, net, env, dev, mains, lg, room_ok):
        import rln36_boss as B

        lgn = lg.float().cpu().numpy().astype(np.float64)
        n, width = lgn.shape
        legal = np.isfinite(lgn)
        kind = kinds(width)
        greedy = lgn.argmax(1)
        z = np.where(legal, lgn, -np.inf)
        p = np.exp(z - z.max(1, keepdims=True))
        p /= p.sum(1, keepdims=True)
        gk = kind[greedy]
        card = legal & (kind == B.KIND_HAND)[None]
        nonpot = legal & (kind != B.KIND_POTION)[None]
        best_card = np.where(card, lgn, -np.inf).argmax(1)
        best_nonpot = np.where(nonpot, lgn, -np.inf).argmax(1)
        force_end = np.zeros(n, dtype=bool)
        maybe = (gk == B.KIND_END) & card.any(1)
        if maybe.any():
            st = row_state(env.obs[mains[maybe]])
            force_end[maybe] = st[:, 0] > 0
        force_pot = (gk == B.KIND_POTION) & nonpot.any(1)
        if not self.force:
            force_end[:], force_pot[:] = False, False
        search = room_ok & (legal.sum(1) > 1) & ((p.max(1) < self.pskip) | force_end | force_pot)
        self.stats["decisions"] += n
        self.stats["force_end"] += int((search & force_end).sum())
        self.stats["force_pot"] += int((search & force_pot).sum())
        self.last_searched = search.astype(np.int32)
        self.last_rows = np.zeros(n, dtype=np.int64)
        top = np.argsort(-p, 1)[:, :TOPN]
        self.last_tgt_a = top.astype(np.int16)
        self.last_tgt_p = np.take_along_axis(p, top, 1).astype(np.float32)
        choice = greedy.copy()
        if not search.any():
            return choice
        rows = np.nonzero(search)[0]
        self.stats["searched"] += len(rows)
        K, m, F = self.kmax, self.m, self.base
        cands, noise = {}, {}
        src, dst, act, seeds = [], [], [], []
        for r in rows:
            g = self.rng.gumbel(size=width) if self.gumbel else np.zeros(width)
            score = np.where(legal[r], g + lgn[r], -np.inf)
            score[greedy[r]] = -np.inf
            nl = int(legal[r].sum())
            c = [int(greedy[r])] + [int(x) for x in np.argsort(-score)[: min(self.k, nl) - 1]]
            if force_end[r] and best_card[r] not in c:
                c.append(int(best_card[r]))
            if force_pot[r] and best_nonpot[r] not in c:
                c.append(int(best_nonpot[r]))
            cands[r], noise[r] = c, g
            f = mains[r]
            ss = self.rng.integers(0, 2**63, size=m, dtype=np.uint64)
            for j, a in enumerate(c):
                base = F + (f * K + j) * m
                src += [f] * m
                dst += list(range(base, base + m))
                act += [a] * m
                seeds.append(ss)
        util, per_slot = self.rollouts(net, env, dev, np.asarray(src), np.asarray(dst),
                                       np.asarray(act), np.concatenate(seeds),
                                       int(mains.max()) + 1, K * m)
        self.last_rows = per_slot[mains]
        for r in rows:
            f, c, g = mains[r], cands[r], noise[r]
            u = np.stack([util[F + (f * K + j) * m:F + (f * K + j + 1) * m] for j in range(len(c))])
            d = u - u[:1]
            se = d.std(1, ddof=1) / np.sqrt(m) if m > 1 else np.zeros(len(c))
            mean = d.mean(1)
            q = u[0].mean() + np.sign(mean) * np.maximum(0.0, np.abs(mean) - self.z * se)
            pc = p[r, c]
            vmix = float((pc * q).sum() / max(pc.sum(), 1e-12))
            cq = np.full(width, vmix)
            cq[c] = q
            t = np.where(legal[r], lgn[r] + self.beta * cq, -np.inf)
            t = np.exp(t - t.max())
            t /= t.sum()
            o = np.argsort(-t)[:TOPN]
            self.last_tgt_a[r] = o
            self.last_tgt_p[r] = t[o]
            pick = c[int(np.argmax(g[c] + lgn[r, c] + self.beta * q))]
            choice[r] = pick
            if pick != greedy[r]:
                self.stats["deviated"] += 1
                if force_end[r] and pick == best_card[r]:
                    self.stats["fixed_end"] += 1
                if force_pot[r] and pick == best_nonpot[r]:
                    self.stats["fixed_pot"] += 1
        return choice


class TargetRecorder(EI.Recorder):
    """`naz3_ei.Recorder` storing the player's targets (`last_tgt_*`) as the top 8, with no
    candidates, so `naz3_ei.load_data` uses them as they are."""

    def step(self, player, env, idx, k_of, lg, a):
        s = player.last_searched.astype(bool)
        sel = s | (self.rng.random(len(idx)) < self.keep)
        if not sel.any():
            return
        rows = idx[sel]
        b = self.buf
        b["obs"].append(env.obs[rows].copy())
        b["masks"].append(env.masks[rows].copy())
        if getattr(player, "env_kw", {}).get("priv"):  # MCTS: the value head's input too
            b.setdefault("priv", []).append(env.priv[rows].copy())
        b["top_a"].append(player.last_tgt_a[sel])
        b["top_p"].append(player.last_tgt_p[sel].astype(np.float16))
        b["cand"].append(np.full((int(sel.sum()), 1), -1, dtype=np.int16))
        b["q"].append(np.full((int(sel.sum()), 1), np.nan, dtype=np.float32))
        b["u"].append(np.full((int(sel.sum()), 1, 1), np.nan, dtype=np.float16))
        b["act"].append(a[sel].astype(np.int16))
        b["fight"].append(k_of[sel].astype(np.int64))
        b["searched"].append(s[sel])
        self.rows += int(sel.sum())
        if self.rows >= self.chunk:
            self.flush()


class Mistakes:
    """Per-decision counts of the targeted mistakes (an `on_step` hook):
    - `end_unused`: the turn ended with energy left while a card was playable;
    - `end_underblock`: the turn ended with block below the visible incoming damage while energy
      was left and a card was playable;
    - `osty_low_end`: the turn ended with Osty alive at <= 3 HP against incoming damage;
    - `potion_early`: a potion drunk while the enemies still have >= 60% of their HP;
    normalised by the turn ends (`ends`) or potions drunk (`potions`)."""

    def __init__(self):
        self.c = collections.Counter()

    def step(self, env, idx, k_of, lg, a):
        import rln36_boss as B

        width = lg.shape[1]
        kind = kinds(width)
        legal = torch_legal(lg)
        ka = kind[a]
        end = ka == B.KIND_END
        pot = ka == B.KIND_POTION
        if end.any():
            st = row_state(env.obs[idx[end]])
            card = (legal[end] & (kind == B.KIND_HAND)[None]).any(1)
            e, blk, inc, osty = st.T
            self.c["ends"] += int(end.sum())
            self.c["end_unused"] += int(((e > 0) & card).sum())
            self.c["end_underblock"] += int(((e > 0) & card & (blk < inc)).sum())
            self.c["osty_low_end"] += int(((osty > 0) & (osty <= 3) & (inc > 0)).sum())
        if pot.any():
            dealt = C.enemy_dealt(env.obs[idx[pot]])
            self.c["potions"] += int(pot.sum())
            self.c["potion_early"] += int((dealt < 0.4).sum())
        self.c["decisions"] += len(idx)

    def rates(self):
        c = self.c
        e, p = max(c["ends"], 1), max(c["potions"], 1)
        return dict(ends=c["ends"], end_unused=c["end_unused"] / e,
                    end_underblock=c["end_underblock"] / e, osty_low_end=c["osty_low_end"] / e,
                    potions=c["potions"], potion_early=c["potion_early"] / p,
                    decisions=c["decisions"])


def torch_legal(lg):
    return np.isfinite(lg.float().cpu().numpy())


def starts(path, mix, count, rng, exclude=()):
    pool = [r for r in C.read_jsonl(path) if r.get("id") not in exclude]
    by = {room: [r for r in pool if C.room_of(r) == room] for room in C.ROOMS}
    out = []
    for room, share in zip(("boss", "elite", "hallway"), mix):
        n = int(round(count * share))
        if n and by[room]:
            out += [by[room][i] for i in rng.choice(len(by[room]), n, replace=len(by[room]) < n)]
    return [out[i] for i in rng.permutation(len(out))]


def player_of(a, gumbel):
    return GumbelPlayer(a.k, a.m, a.beta, a.z, a.pskip, gumbel=gumbel, force=not a.no_force,
                        loss_w=a.loss_w, seed=a.seed)


def cmd_gen(a):
    net, dev = C.net_and_dev(a.ckpt, a.weights)
    rng = np.random.default_rng(a.seed)
    bench = {r["id"] for r in C.read_jsonl(C.DATA / "bench.jsonl")}
    recs = starts(pathlib.Path(a.starts).expanduser(), [float(x) for x in a.mix.split(",")],
                  a.count, rng, bench)
    player = player_of(a, not a.no_gumbel)
    rec = TargetRecorder(a.out, a.keep, a.chunk, a.seed)
    mis = Mistakes()
    off = a.seed * 10_000_000

    def on_step(env, idx, k_of, lg, act):
        rec.step(player, env, idx, k_of + off, lg, act)
        mis.step(env, idx, k_of, lg, act)

    t0 = time.time()
    res = C.play(net, dev, recs, 1, a.seed, a.fights, player, a.threads, on_step=on_step,
                 on_end=lambda k, w, hp, mx: rec.end(k + off, w, hp, mx))
    rec.close(off)
    meta = dict(ckpt=str(a.ckpt), weights=a.weights, starts=a.starts, count=a.count, seed=a.seed,
                mix=a.mix, k=a.k, m=a.m, beta=a.beta, z=a.z, pskip=a.pskip, loss_w=a.loss_w,
                keep=a.keep, wall_s=time.time() - t0, stats=dict(player.stats),
                died=float(res["died"].mean()), mistakes=mis.rates())
    (pathlib.Path(a.out) / f"meta{off:09d}.json").write_text(json.dumps(meta))
    print(f"gen: {len(recs)} fights in {meta['wall_s']:.0f}s, died {meta['died']:.3f}, "
          f"{meta['stats']}, mistakes {meta['mistakes']}", flush=True)
    return 0


def cmd_mistakes(a):
    net, dev = C.net_and_dev(a.ckpt, a.weights)
    recs = C.read_jsonl(pathlib.Path(a.bench).expanduser())
    if a.room:
        recs = [r for r in recs if C.room_of(r) == a.room]
    player = player_of(a, False) if a.search else None
    mis = Mistakes()
    res = C.play(net, dev, recs, a.reps, 7_000_000, a.fights, player, a.threads,
                 on_step=mis.step)
    out = dict(ckpt=str(a.ckpt), search=a.search, room=a.room, reps=a.reps,
               died=float(res["died"].mean()), **mis.rates(),
               stats=dict(player.stats) if player else {})
    pathlib.Path(a.out).write_text(json.dumps(out, indent=1))
    print(json.dumps(out), flush=True)
    return 0


def search_flags(p):
    p.add_argument("--k", type=int, default=4)
    p.add_argument("--m", type=int, default=8)
    p.add_argument("--beta", type=float, default=10.0)
    p.add_argument("--z", type=float, default=1.0)
    p.add_argument("--pskip", type=float, default=0.95)
    p.add_argument("--loss-w", type=float, default=0.5)
    p.add_argument("--no-force", action="store_true", help="no forced mistake alternatives")
    p.add_argument("--seed", type=int, default=0)
    p.add_argument("--fights", type=int, default=512)
    p.add_argument("--threads", type=int, default=12)
    p.add_argument("--weights", default="raw")


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    g = sub.add_parser("gen")
    g.add_argument("ckpt")
    g.add_argument("--starts", default="~/.cache/spirebot-data/cb1/train_h.jsonl")
    g.add_argument("--count", type=int, required=True)
    g.add_argument("--mix", default="0.5,0.35,0.15", help="boss, elite, hallway shares")
    g.add_argument("--keep", type=float, default=0.3, help="share of unsearched rows stored")
    g.add_argument("--chunk", type=int, default=100_000)
    g.add_argument("--no-gumbel", action="store_true")
    g.add_argument("--out", required=True)
    search_flags(g)
    mk = sub.add_parser("mistakes")
    mk.add_argument("ckpt")
    mk.add_argument("--bench", default=str(C.DATA / "bench.jsonl"))
    mk.add_argument("--room", default="boss")
    mk.add_argument("--reps", type=int, default=2)
    mk.add_argument("--search", action="store_true")
    mk.add_argument("--out", required=True)
    search_flags(mk)
    if len(sys.argv) > 1 and sys.argv[1] == "train":  # naz3_ei's trainer, unchanged
        sys.argv[0] = "naz3_ei.py"
        return EI.main()
    a = p.parse_args()
    return {"gen": cmd_gen, "mistakes": cmd_mistakes}[a.cmd](a)


if __name__ == "__main__":
    raise SystemExit(main())
