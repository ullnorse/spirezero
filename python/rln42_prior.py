"""RLN42: NECRO4's run decisions from the RLN41 prior (`docs/research/rln42-necro4.md`).

    python python/rln42_prior.py collect CKPT --runs 2000 --out target/rln42/states   # states
    python python/rln42_prior.py distill CKPT --data target/rln42/states --out D.pt     # warm-up
    python python/rln42_prior.py probe CKPT [--weights ema0.999] [--runs 1000]          # decisions

**Option sets** (`row_entries`): one run obs row and its legal mask give the RLN41 records it
offers, each as a decision kind, its options [(token type, id, upgrade)] and per action the option
it takes (-1 outside the set; several actions may take one option). Built from the legal actions
only, so the prior and the policy share the support:
- `rest`: HEAL / SMITH / OTHER over the enabled, legal rest slots;
- `card_reward`: the first card reward with a legal pick: its cards + SKIP (skip, its two
  alternatives and PROCEED, as the RLN41 adapter counts a choice);
- `smith_target` / `shop_remove`: a select of that purpose, one option per distinct (card, upgrade);
- `shop_card`, `shop_relic`: the buyable cards / relics + NONE (PROCEED); `shop_removal`: on the
  visit's first row only (the human record is "used the removal at this visit"), REMOVE vs NONE =
  every other legal shop action (buys, potions, leave), when the removal is legal (affordable,
  unused). Asked at every step against PROCEED alone, it let the gold go to buys first: 1B buys
  at 99.9% of its shop rows with a removal affordable (RLN42).
- `ancient`, `event`: the legal options.
A shop row gives up to three entries. `prior_inputs` builds the prior's tensors from the obs bytes
directly (vectorised; equal to `rln41_prior.collate` of the adapter's records, `test_rln42.py`).

**KL to the prior** (`PriorKL`, `--set prior={...}`): during collection each run row's entries
are found while the GPU runs the forward; after the batch the prior scores them (one GPU call) into
a table, and each row keeps up to 3 entry ids (`pent`). The update adds, per row,
sum over entries of beta_kind * KL(pi_M || p), with pi_M the policy renormalised over the
entry's options (an option's mass is the sum over its actions); other actions and combat rows are
untouched. beta = `coef` until `flat` decisions, linear to `decay_to` at `decay_steps`, then held;
kinds in `kind_scale` (smith_target, shop_card: 1/4, RLN41) are scaled.

**Distillation** (`distill`): the warm-started net (NECRO3 1B `ema0.999`, `run_head` and the value
head re-initialised); the trunk, the combat head and the value module frozen; the run head trained on
run rows collected by `collect` (greedy 1B runs, and runs that follow the prior's argmax where it
has one and 1B elsewhere). Loss per row: CE(prior, pi_M) per entry plus KL(teacher || pi) on the
partition {each action outside the entries, their union as one bin} (the teacher is the shared
head, i.e. 1B's own run policy: map moves, claims and unmapped events keep 1B's choices).
"""

import argparse
import collections
import dataclasses
import json
import pathlib
import sys
import time

import numpy as np
import torch

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import rln41_prior as P  # noqa: E402

D = P.D
KINDS = D.KINDS
KIND_ID = P.KIND_ID
N_ACT = 361
K_MAX = 80  # a select's distinct cards (80 deck slots)
SLOTS = 3  # entries per row (a shop: card, relic, removal)
MAPPED = (P.PHASE_ANCIENT, P.PHASE_EVENT, P.PHASE_REWARDS, P.PHASE_SELECT, P.PHASE_REST,
          P.PHASE_SHOP)
SELECT_KIND = {P.SELECT_SMITH: "smith_target", P.SELECT_SHOP_REMOVAL: "shop_remove"}
ALT_BASE = P.ALTERNATIVE_BASE


def legal_bits(masks):
    """(n, 6) uint64 masks -> (n, 384) bool, bit a = action a."""
    m = np.ascontiguousarray(masks).view(np.uint8).reshape(len(masks), -1)
    return np.unpackbits(m, axis=1, bitorder="little").astype(bool)


def _u16(o, at):
    return int(o[at]) | int(o[at + 1]) << 8


def row_entries(o, legal, shop_entry=False):
    """[(kind id, options, action -> option int8 (N_ACT,))] of one run obs row (module doc);
    `shop_entry`: the row is its shop visit's first."""
    phase = int(o[3])
    out = []

    def add(kind, opts, acts):
        if len(opts) < 2:
            return
        a2o = np.full(N_ACT, -1, np.int8)
        for j, al in enumerate(acts):
            a2o[al] = j
        out.append((KIND_ID[kind], opts, a2o))

    if phase == P.PHASE_REST:
        toks, acts = [], []
        for i in range(P.OPT_SLOTS):
            k, en = int(o[P.REST_AT + 2 * i]) - 1, o[P.REST_AT + 2 * i + 1]
            if k < 0 or not en or not legal[P.REST_BASE + i]:
                continue
            t = P.REST_KIND.get(k, D.T_OTHER)
            if t not in toks:
                toks.append(t)
                acts.append([])
            acts[toks.index(t)].append(P.REST_BASE + i)
        add("rest", [(t, 0, 0) for t in toks], acts)
    elif phase == P.PHASE_REWARDS:
        for r, cs in P.card_rewards(o):
            lo = P.PICK_CARD_BASE + 5 * r
            picks = [(j, c) for j, c in enumerate(cs) if legal[lo + j]]
            if not picks:
                continue
            skip = [a for a in (P.SKIP_BASE + r, ALT_BASE + 2 * r, ALT_BASE + 2 * r + 1, P.PROCEED)
                    if legal[a]]
            opts = [(D.T_CARD, c, u) for _, (c, u) in picks]
            acts = [[lo + j] for j, _ in picks]
            if skip:
                opts.append((D.T_SKIP, 0, 0))
                acts.append(skip)
            add("card_reward", opts, acts)
            break
    elif phase == P.PHASE_SELECT:
        kind = SELECT_KIND.get(int(o[P.SELECT_AT]))
        if kind:
            at = P.SELECT_AT + 8
            groups = {}
            for i in range(80):
                c = _u16(o, at + 5 * i)
                if c and legal[P.SELECT_BASE + i]:
                    groups.setdefault((c, int(o[at + 5 * i + 2])), []).append(P.SELECT_BASE + i)
            keys = sorted(groups)
            add(kind, [(D.T_CARD, c, u) for c, u in keys], [groups[k] for k in keys])
    elif phase == P.PHASE_SHOP:
        none = [P.PROCEED] if legal[P.PROCEED] else []
        opts, acts = [], []
        for i in range(8):
            c = _u16(o, P.SHOP_AT + 6 * i)
            if c and legal[P.BUY_CARD_BASE + i]:
                opts.append((D.T_CARD, c, int(o[P.SHOP_AT + 6 * i + 2])))
                acts.append([P.BUY_CARD_BASE + i])
        if opts and none:
            add("shop_card", opts + [(D.T_NONE, 0, 0)], acts + [none])
        opts, acts = [], []
        for j in range(4):
            r = _u16(o, P.SHOP_AT + 48 + 4 * j)
            if r and legal[P.BUY_RELIC_BASE + j]:
                opts.append((D.T_RELIC, r, 0))
                acts.append([P.BUY_RELIC_BASE + j])
        if opts and none:
            add("shop_relic", opts + [(D.T_NONE, 0, 0)], acts + [none])
        if shop_entry and legal[P.REMOVE_CARD]:
            other = [a for a in np.nonzero(legal[:N_ACT])[0] if a != P.REMOVE_CARD]
            add("shop_removal", [(D.T_REMOVE, 0, 0), (D.T_NONE, 0, 0)], [[P.REMOVE_CARD], other])
    elif phase == P.PHASE_ANCIENT:
        opts, acts = [], []
        for i in range(P.OPT_SLOTS):
            r = _u16(o, P.ANCIENT_AT + 2 + 3 * i)
            if r and not o[P.ANCIENT_AT + 4 + 3 * i] and legal[P.ANCIENT_BASE + i]:
                opts.append((D.T_RELIC, r, 0))
                acts.append([P.ANCIENT_BASE + i])
        add("ancient", opts, acts)
    elif phase == P.PHASE_EVENT:
        opts, acts = [], []
        for i in range(P.OPT_SLOTS):
            k = _u16(o, P.EVENT_AT + 2 + 3 * i)
            if k and o[P.EVENT_AT + 4 + 3 * i] and legal[P.EVENT_BASE + i]:
                opts.append((D.T_EVENT, k, 0))
                acts.append([P.EVENT_BASE + i])
        add("event", opts, acts)
    return out


@dataclasses.dataclass
class Scan:
    """Entries found on some rows: `row` (the row index each came from), `kind`, `opts`,
    `a2o` (E, N_ACT) int8 and `obs` (E, W) uint8 (a copy of the row)."""
    row: np.ndarray
    kind: np.ndarray
    opts: list
    a2o: np.ndarray
    obs: np.ndarray

    def __len__(self):
        return len(self.row)


def scan(obs, masks, rows=None, entry=None):
    """The entries of the run rows of `obs` (n, W) with `masks` (n, 6) uint64 (a phase in
    `MAPPED`; combat rows have phase `PHASE_COMBAT`); `rows` restricts it to those indices;
    `entry` (n,) bool: the row is its shop visit's first (`shop_entries`)."""
    idx = np.arange(len(obs)) if rows is None else np.asarray(rows)
    idx = idx[np.isin(obs[idx, 3], MAPPED)]
    legal = legal_bits(masks[idx]) if len(idx) else np.zeros((0, 384), bool)
    row, kind, opts, a2o = [], [], [], []
    for j, i in enumerate(idx):
        for k, op, a in row_entries(obs[i], legal[j], entry is not None and bool(entry[i])):
            row.append(i)
            kind.append(k)
            opts.append(op)
            a2o.append(a)
    row = np.asarray(row, np.int64)
    return Scan(row=row, kind=np.asarray(kind, np.int64), opts=opts,
                a2o=np.stack(a2o) if a2o else np.zeros((0, N_ACT), np.int8),
                obs=obs[row].copy())


class ShopEntries:
    """Which rows are a shop visit's first: per env the phase of its previous decision (back
    from the removal's card select is the same visit)."""

    def __init__(self, n):
        self.prev = np.full(n, 255, np.uint8)

    def __call__(self, obs, rows=None):
        idx = np.arange(len(obs)) if rows is None else np.asarray(rows)
        ph = obs[idx, 3]
        out = np.zeros(len(obs), bool)
        prev = self.prev[idx]
        out[idx] = (ph == P.PHASE_SHOP) & (prev != P.PHASE_SHOP) & (prev != P.PHASE_SELECT)
        self.prev[idx] = ph
        return out

    def ended(self, i):
        self.prev[i] = 255


def prior_inputs(obs, kinds, opts):
    """`rln41_prior.collate`'s tensors for entries on obs rows (E, W) uint8 (module doc)."""
    e = len(kinds)
    o = obs.astype(np.int64)
    d = o[:, P.DECK_AT:P.DECK_AT + 320].reshape(e, 80, 4)
    ids = d[..., 0] | d[..., 1] << 8
    ups = np.where(ids > 0, d[..., 2], 0)
    r = o[:, P.RELICS_AT:P.RELICS_AT + 128].reshape(e, 32, 4)
    rel = r[..., 0] | r[..., 1] << 8

    def s16(at):
        v = o[:, at] | o[:, at + 1] << 8
        return np.where(v >= 32768, v - 65536, v)
    hp, mhp, gold = s16(8), np.maximum(1, s16(10)), s16(12)
    act = np.clip(o[:, 2], 0, 2)
    floor = o[:, 7] + P.FLOOR_OFFSET
    scal = np.stack([hp / mhp, mhp / 100, gold / 300, act == 0, act == 1, act == 2, floor / 50,
                     (ids > 0).sum(1) / 30, (rel > 0).sum(1) / 15], 1).astype(np.float32)
    k = max(len(x) for x in opts)
    opt = np.zeros((e, k, 3), np.int64)
    omask = np.zeros((e, k), bool)
    for i, x in enumerate(opts):
        opt[i, :len(x)] = x
        omask[i, :len(x)] = True
    cnt = ((opt[:, :, 1, None] == ids[:, None, :]) & (ids[:, None, :] > 0)).sum(-1)
    cnt = (cnt * (opt[:, :, 0] == D.T_CARD)).astype(np.float32)
    t = torch.from_numpy
    return dict(deck=t(np.stack([ids, ups], -1)), rel=t(rel), scal=t(scal),
                kind=t(np.asarray(kinds, np.int64)), opt=t(opt), cnt=t(cnt), omask=t(omask))


@torch.no_grad()
def prior_logp(model, obs, kinds, opts, device, chunk=8192):
    """(E, K_MAX) float32 log-probs of each entry's options (-inf past them), on `device`."""
    out = torch.full((len(kinds), K_MAX), float("-inf"), device=device)
    for a in range(0, len(kinds), chunk):
        b = slice(a, a + chunk)
        x = {k: v.to(device) for k, v in prior_inputs(obs[b], kinds[b], opts[b]).items()}
        lp = torch.log_softmax(model(x).float(), -1).masked_fill(~x["omask"], float("-inf"))
        out[b, :lp.shape[1]] = lp
    return out


def option_logq(logp, a2o):
    """log pi over each entry's options: `logp` (P, A) the policy's log-probs (-inf off the
    mask), `a2o` (P, A) the option of each action (-1 outside). (P, K_MAX), -inf for options
    with no legal action. The mass of an option is the sum over its actions, renormalised over
    the options (computed from the max in-set log-prob, so a set the policy avoids stays finite)."""
    a2o = a2o[:, :logp.shape[1]].long()  # 355 wide without `discard_potion` (no option there)
    inset = (a2o >= 0) & torch.isfinite(logp)
    x = torch.where(inset, logp, torch.full_like(logp, float("-inf")))
    mx = x.max(1, keepdim=True).values.detach()
    p = torch.where(inset, (x - mx).exp(), torch.zeros_like(x))
    bins = torch.where(a2o >= 0, a2o, torch.full_like(a2o, K_MAX))
    po = torch.zeros(p.shape[0], K_MAX + 1, device=p.device, dtype=p.dtype).scatter_add(1, bins, p)
    po = po[:, :K_MAX]
    lq = torch.log(po.clamp(min=1e-30)) - torch.log(po.sum(1, keepdim=True))
    return torch.where(po > 0, lq, torch.full_like(lq, float("-inf")))


def _finite(x):
    """`x` with its -inf entries 0 (a constant: no gradient through them, no 0 * inf)."""
    return torch.where(torch.isfinite(x), x, torch.zeros_like(x))


def kl_policy_prior(logq, lp):
    """KL(q || p) per entry over the options where q > 0 (both (P, K_MAX) log-probs)."""
    on = torch.isfinite(logq)
    lq = _finite(logq)
    term = torch.where(on, lq.exp() * (lq - _finite(lp.clamp(min=-1e4))), torch.zeros_like(lq))
    return term.sum(1)


def ce_prior_policy(logq, lp):
    """CE(p, q) per entry: -sum p log q over the options."""
    on = torch.isfinite(lp)
    p = _finite(lp).exp() * on
    return -(p * _finite(logq).clamp(min=-1e4)).sum(1)


# ---------------------------------------------------------------------------------------------
# KL during RL


@dataclasses.dataclass
class PriorCfg:
    path: str = str(P.DATA / "prior.pt")
    coef: float = 0.1
    decay_to: float = 0.02
    # NECRO4 (owner, 2026-11-12): held longer than RLN41's 50M -> 250M, combat learns slowly.
    flat: int = 250_000_000
    decay_steps: int = 500_000_000
    kind_scale: dict = dataclasses.field(
        default_factory=lambda: {"smith_target": 0.25, "shop_card": 0.25})
    keep: int = 8  # collects whose entries stay in the table (held rows reach back <= hold_iters)
    # NECRO4 at 30-37M: the KL renormalises pi over a select's cards, so `select_cancel` was free
    # and the policy looped rest -> SMITH -> cancel (47 rest rows per run, combat share 0.86 ->
    # 0.53, held rows 40k -> 330k, CUDA OOM). `leave` * -log pi(the entry's options) on the
    # `LEAVE_KINDS` entries, constant (no decay): the prior has no "back out" option.
    leave: float = 0.1

    @classmethod
    def from_dict(cls, d):
        return None if d is None else cls(**d)

    def beta(self, decisions):
        if decisions <= self.flat:
            return self.coef
        f = min(1.0, (decisions - self.flat) / max(1, self.decay_steps - self.flat))
        return self.coef + (self.decay_to - self.coef) * f


LEAVE_KINDS = ("smith_target", "shop_remove")


class PriorKL:
    """The entry table of the last `keep` collects (`a2o`, prior log-probs, kind, scale) on the
    device; ids are global and increasing, `base` the first one kept."""

    def __init__(self, cfg, device):
        self.cfg = cfg
        self.dev = device
        self.model = P.load_model(pathlib.Path(cfg.path).expanduser()).to(device)
        self.scale = torch.tensor([cfg.kind_scale.get(k, 1.0) for k in KINDS], device=device)
        self.leave = torch.tensor([cfg.leave if k in LEAVE_KINDS else 0.0 for k in KINDS],
                                  device=device)
        self.parts = []  # (first id, n) per collect
        self.next_id = 0
        self.base = 0
        self.a2o = torch.zeros(0, N_ACT, dtype=torch.int8, device=device)
        self.lp = torch.zeros(0, K_MAX, device=device)
        self.kind = torch.zeros(0, dtype=torch.long, device=device)

    def commit(self, scans):
        """Scores the collect's entries ([(t, Scan)]); returns (t, env, slot, id) arrays."""
        scans = [(t, s) for t, s in scans if len(s)]
        ts, rows, slots, kinds, opts, a2o, obs = [], [], [], [], [], [], []
        for t, s in scans:
            ts.append(np.full(len(s), t))
            rows.append(s.row)
            _, first, inv = np.unique(s.row, return_index=True, return_inverse=True)
            slots.append(np.arange(len(s)) - first[inv])  # a row's entries are consecutive
            kinds.append(s.kind)
            opts += s.opts
            a2o.append(s.a2o)
            obs.append(s.obs)
        if not scans:
            z = np.zeros(0, np.int64)
            return z, z, z, z
        ts, rows, slots, kinds = (np.concatenate(x) for x in (ts, rows, slots, kinds))
        keep = slots < SLOTS
        lp = prior_logp(self.model, np.concatenate(obs), kinds, opts, self.dev)
        n = len(kinds)
        ids = np.arange(self.next_id, self.next_id + n)
        self.parts.append((self.next_id, n))
        self.next_id += n
        drop = max(0, len(self.parts) - self.cfg.keep)
        cut = sum(m for _, m in self.parts[:drop])
        self.parts = self.parts[drop:]
        self.base = self.parts[0][0]
        self.a2o = torch.cat([self.a2o[cut:], torch.from_numpy(np.concatenate(a2o)).to(self.dev)])
        self.lp = torch.cat([self.lp[cut:], lp])
        self.kind = torch.cat([self.kind[cut:], torch.from_numpy(kinds).to(self.dev)])
        return ts[keep], rows[keep], slots[keep], ids[keep]

    def kl(self, logp, pent, beta):
        """Per row (m,) the sum over its entries of beta_kind * KL(pi_M || p); `logp` (m, A) the
        policy's log-probs, `pent` (m, SLOTS) entry ids (-1 none). Also (kind ids, KL) detached."""
        ids = pent.long()
        r, s = ((ids >= self.base) & (ids < self.next_id)).nonzero(as_tuple=True)
        out = torch.zeros(logp.shape[0], device=logp.device)
        if r.numel() == 0:
            return out, None
        e = ids[r, s] - self.base
        lr, a2o = logp[r], self.a2o[e]
        kl = kl_policy_prior(option_logq(lr, a2o), self.lp[e])
        kind = self.kind[e]
        inset = (a2o[:, :lr.shape[1]] >= 0) & torch.isfinite(lr)
        lin = torch.logsumexp(torch.where(inset, lr, torch.full_like(lr, float("-inf"))), 1)
        out = out.index_add(0, r, beta * self.scale[kind] * kl - self.leave[kind] * _finite(lin))
        return out, (kind, kl.detach())


class KindStats:
    """KL per kind and the decision rates of a collect (logged per iteration)."""

    def __init__(self, dev):
        self.kl = torch.zeros(2, len(KINDS), dtype=torch.float64, device=dev)

    def add_kl(self, kind_kl):
        if kind_kl is None:
            return
        kind, kl = kind_kl
        self.kl[0].index_add_(0, kind, kl.double())
        self.kl[1].index_add_(0, kind, torch.ones_like(kl, dtype=torch.float64))

    def kl_means(self):
        s, n = self.kl.cpu().numpy()
        return {k: round(float(s[i] / n[i]), 5) for i, k in enumerate(KINDS) if n[i] > 0}


def choice_stats(out, sc, actions):
    """Tallies a step's choices (the sampled `actions` per env) on its entries into `out`:
    Smith share of rest choices (all, HP >= 70%), skip share, removal use."""
    if not len(sc):
        return
    o = sc.obs.astype(np.int64)
    hp = (o[:, 8] | o[:, 9] << 8) / np.maximum(1, o[:, 10] | o[:, 11] << 8)
    for j in range(len(sc)):
        ch = int(sc.a2o[j, actions[sc.row[j]]])
        if ch < 0:
            continue
        kind, opts = KINDS[sc.kind[j]], sc.opts[j]
        t = opts[ch][0]
        if kind == "rest" and {D.T_SMITH, D.T_HEAL} <= {x[0] for x in opts}:
            key = "rest_hi" if hp[j] >= 0.7 else "rest_lo"
            c = out.setdefault(key, [0, 0])
            c[0] += t == D.T_SMITH
            c[1] += 1
        elif kind == "card_reward":
            c = out.setdefault("card_reward", [0, 0])
            c[0] += t == D.T_SKIP
            c[1] += 1
        elif kind == "shop_removal":
            c = out.setdefault("shop_removal", [0, 0])
            c[0] += t == D.T_REMOVE
            c[1] += 1


def rates(c):
    """The logged rates from `choice_stats` counts."""
    def r(*keys):
        a = sum(c.get(k, [0, 0])[0] for k in keys)
        b = sum(c.get(k, [0, 0])[1] for k in keys)
        return None if b == 0 else round(a / b, 4)
    return dict(smith=r("rest_hi", "rest_lo"), smith_hi=r("rest_hi"), skip=r("card_reward"),
                removal=r("shop_removal"))


# ---------------------------------------------------------------------------------------------
# Distillation states, the warm-up, the probe


def _policy(args, device):
    import e2e_eval as V
    return V.load_policy(args.ckpt, args.weights, device)


def _env(net, seeds, threads):
    import rln40_smith as S
    return S.make_env(len(seeds), seeds, net, threads)


@torch.no_grad()
def _logits(net, env, idx, flags, device):
    from sts2.e2e import encode as E
    v2, full, events = flags
    obs = torch.from_numpy(env.obs[idx]).to(device)
    masks = torch.from_numpy(env.masks.view(np.int64)[idx]).to(device)
    tok = E.encode(obs, masks, v2=v2, full=full, events=events,
                   discard=bool(getattr(net, "env_opts", {}).get("discard_potion")))
    with torch.autocast(device.type, dtype=torch.bfloat16, enabled=device.type == "cuda"):
        logits, _, _ = net.policy(tok)
    return logits.float(), tok["combat"]


def play(net, seeds, device, threads, prior=None, on_row=None, on_end=None):
    """Greedy runs of `seeds`; with `prior` (a model) the prior's argmax replaces the net's choice
    on rows with an entry: a shop visit's removal choice first, then one of the row's other entries
    at random (so the removal's card choice is reached as often as the buys). `on_row(i, obs, mask, combat, action)` sees
    every decision; `on_end(i, env)` every finished run."""
    from sts2 import HOLD
    env, flags = _env(net, seeds, threads)
    n = len(seeds)
    done = np.zeros(n, dtype=bool)
    rng = np.random.default_rng(int(seeds[0]))
    shops = ShopEntries(n)
    try:
        while not done.all():
            live = np.nonzero(~done)[0]
            logits, combat = _logits(net, env, live, flags, device)
            act = logits.argmax(-1).cpu().numpy()
            combat = combat.cpu().numpy()
            entry = shops(env.obs, live)
            if prior is not None:
                sc = scan(env.obs, env.masks, live[~combat], entry)
                if len(sc):
                    lp = prior_logp(prior, sc.obs, sc.kind, sc.opts, device).cpu().numpy()
                    pos = {int(i): j for j, i in enumerate(live)}
                    rows = collections.defaultdict(list)
                    for j in range(len(sc)):
                        rows[int(sc.row[j])].append(j)
                    rm = KINDS.index("shop_removal")
                    for i, js in rows.items():
                        # the visit's removal first: REMOVE acts, NONE leaves the other entries
                        r = [j for j in js if sc.kind[j] == rm]
                        if r and int(np.argmax(lp[r[0]])) == 0:
                            act[pos[i]] = P.REMOVE_CARD
                            continue
                        js = [j for j in js if sc.kind[j] != rm]
                        if not js:
                            continue
                        j = js[rng.integers(len(js))]
                        best = int(np.argmax(lp[j]))
                        act[pos[i]] = int(np.nonzero(sc.a2o[j] == best)[0][0])
            if on_row is not None:
                for j, i in enumerate(live):
                    on_row(i, env.obs[i], env.masks[i], bool(combat[j]), int(act[j]),
                           bool(entry[i]))
            step = np.full(n, HOLD, dtype=np.uint16)
            step[live] = act
            env.step(step)
            for i in live[env.done[live].astype(bool)]:
                done[i] = True
                shops.ended(i)
                if on_end is not None:
                    on_end(i, env)
    finally:
        env.close()


COLLECT_SEEDS = 190_000_000  # the training range's top (NECRO3's runs start at 100M)


def cmd_collect(args):
    """Run rows of greedy 1B runs and of prior-following runs -> obs.npy, masks.npy, run.npy."""
    torch.set_num_threads(2)
    device = torch.device(args.device)
    net = _policy(args, device)
    prior = P.load_model(pathlib.Path(args.prior).expanduser()).to(device)
    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    obs, masks, run, src, entry = [], [], [], [], []
    t0 = time.time()
    for mode in args.modes.split(","):
        first = COLLECT_SEEDS + (0 if mode == "greedy" else 2_000_000)
        for b in range(0, args.runs, args.batch):
            seeds = list(range(first + b, first + min(args.runs, b + args.batch)))

            def on_row(i, o, m, combat, a, ent, seeds=seeds, mode=mode):
                if not combat:
                    obs.append(o.copy())
                    masks.append(m.copy())
                    run.append(seeds[i])
                    src.append(mode == "prior")
                    entry.append(ent)
            play(net, seeds, device, args.threads, prior if mode == "prior" else None, on_row)
            print(f"{mode} {b + len(seeds)} runs, {len(obs)} run rows, {time.time() - t0:.0f}s",
                  flush=True)
    np.save(out / "obs.npy", np.stack(obs))
    np.save(out / "masks.npy", np.stack(masks))
    np.save(out / "run.npy", np.asarray(run, np.int64))
    np.save(out / "src.npy", np.asarray(src, bool))
    np.save(out / "entry.npy", np.asarray(entry, bool))
    print(f"-> {out}: {len(obs)} rows")


def split_heldout(run, frac=0.1):
    """Held-out rows: whole runs, every 10th seed."""
    return (run % round(1 / frac)) == 7


def distill_rows(model, obs, masks, entry, device):
    """Entries of the rows (scan) and their prior log-probs: (Scan, lp (E, K) on device)."""
    sc = scan(obs, masks, entry=entry)
    return sc, prior_logp(model, sc.obs, sc.kind, sc.opts, device)


def cmd_distill(args):
    import train_e2e as T
    from sts2.e2e import encode as E

    torch.manual_seed(42)
    device = torch.device(args.device)
    data = pathlib.Path(args.data)
    obs, masks = np.load(data / "obs.npy"), np.load(data / "masks.npy")
    run = np.load(data / "run.npy")
    held = split_heldout(run)
    model = P.load_model(pathlib.Path(args.prior).expanduser()).to(device)
    if args.ckpt == "scratch":  # NECRO4: a random net of `--set` config, all of it trainable
        over = {k: json.loads(v) for k, v in (kv.split("=", 1) for kv in args.set)}
        cfg = T.Config.from_dict({**dataclasses.asdict(T.Config()), **over, "run_head": True})
        torch.manual_seed(cfg.seed)  # as `Learner` builds its net
        net = T.net_of(cfg).to(device)
        value = {id(p) for p in net.value_parameters()}
        for p in net.parameters():
            p.requires_grad_(id(p) not in value)
        print(f"scratch net: layers {cfg.layers} d {cfg.d}", flush=True)
    else:  # the warm start (RLN42 first design): only the run head trains
        src = torch.load(args.ckpt, map_location="cpu", weights_only=False)
        cfg = T.Config.from_dict({**src["config"], "run_head": True, "init_from": str(args.ckpt),
                                  "init_weights": args.weights,
                                  "init_reset": ["run_head.", "v_hidden.", "v_out."]})
        net = T.net_of(cfg)
        report = T.warm_start(net, cfg)
        net = net.to(device)
        print("warm start:", {k: len(v) for k, v in report.items()}, flush=True)
        for name, p in net.named_parameters():
            p.requires_grad_(name.startswith(net.RUN_HEAD_PREFIX))
    head = [p for p in net.parameters() if p.requires_grad]  # the trained parameters
    opt = torch.optim.AdamW(head, lr=args.lr, weight_decay=1e-4)
    t0 = time.time()
    sc, lp = distill_rows(model, obs, masks, np.load(data / "entry.npy"), device)
    print(f"{len(obs)} rows ({held.sum()} held out), {len(sc)} entries, scan+prior "
          f"{time.time() - t0:.0f}s", flush=True)
    # Per row: its entries (indices into sc) for the loss.
    ent_of = collections.defaultdict(list)
    for j, i in enumerate(sc.row):
        ent_of[int(i)].append(j)
    a2o_all = torch.from_numpy(sc.a2o).to(device)
    flags = dict(v2=cfg.obs_v2, full=cfg.obs_full, events=cfg.obs_events, discard=cfg.discard_potion)
    width = obs.shape[1]

    def batch(rows):
        o = torch.from_numpy(obs[rows, :width]).to(device)
        m = torch.from_numpy(masks[rows].view(np.int64)).to(device)
        tok = E.encode(o, m, **flags)
        trunk_grad = torch.is_grad_enabled() and net.ids.weight.requires_grad
        with torch.set_grad_enabled(trunk_grad), torch.autocast(
                device.type, dtype=torch.bfloat16, enabled=device.type == "cuda"):
            h, _ = net.trunk(tok)
        with torch.no_grad(), torch.autocast(device.type, dtype=torch.bfloat16,
                                             enabled=device.type == "cuda"):
            teach = net.head(h.detach(), tok, run=False).float()
        with torch.autocast(device.type, dtype=torch.bfloat16, enabled=device.type == "cuda"):
            stud = net.head(h, tok).float()
        return tok, teach, stud

    def losses(rows, teach, stud):
        ls, lt = torch.log_softmax(stud, -1), torch.log_softmax(teach, -1)
        pos = [(k, j) for k, i in enumerate(rows) for j in ent_of.get(int(i), ())]
        legal = torch.isfinite(lt)
        union = torch.zeros_like(legal)
        ce = torch.zeros((), device=device)
        if pos:
            k = torch.tensor([p[0] for p in pos], device=device)
            j = torch.tensor([p[1] for p in pos], device=device)
            union[k] |= a2o_all[j] >= 0
            ce = ce_prior_policy(option_logq(ls[k], a2o_all[j]), lp[j]).sum() / len(rows)
        union &= legal
        lt0, ls0 = _finite(lt), _finite(ls)
        out_t = (lt0.exp() * (lt0 - ls0) * (legal & ~union)).sum(1)
        has = union.any(1)
        in_t = torch.zeros_like(out_t)
        if has.any():
            u = union[has]
            neg = torch.full_like(lt0[has], float("-inf"))
            lu_t = torch.logsumexp(torch.where(u, lt0[has], neg), 1)
            lu_s = torch.logsumexp(torch.where(u, ls0[has], neg), 1)
            in_t = in_t.index_put((has.nonzero().squeeze(1),), lu_t.exp() * (lu_t - lu_s))
        return ce, (out_t + in_t).mean()

    train = ~held
    if args.teacher == 0:  # only the rows with an entry carry a loss
        train &= np.isin(np.arange(len(obs)), sc.row)
    train_rows, test_rows = np.nonzero(train)[0], np.nonzero(held)[0]
    for ep in range(args.epochs):
        rng = np.random.default_rng(ep)
        perm = rng.permutation(train_rows)
        tot = collections.Counter()
        for a in range(0, len(perm), args.batch):
            rows = np.sort(perm[a:a + args.batch])
            _, teach, stud = batch(rows)
            ce, kt = losses(rows, teach, stud)
            loss = ce + args.teacher * kt
            opt.zero_grad(set_to_none=True)
            loss.backward()
            torch.nn.utils.clip_grad_norm_(head, 1.0)
            opt.step()
            tot["ce"] += float(ce.detach()) * len(rows)
            tot["kt"] += float(kt.detach()) * len(rows)
            tot["n"] += len(rows)
        for g in opt.param_groups:
            g["lr"] *= args.lr_decay
        agree = evaluate(net, test_rows, sc, lp, batch, ent_of)
        print(f"epoch {ep + 1}: ce {tot['ce'] / tot['n']:.4f} teacher-kl {tot['kt'] / tot['n']:.4f} "
              f"({time.time() - t0:.0f}s) held-out agreement {json.dumps(agree)}", flush=True)
    for p in net.parameters():
        p.requires_grad_(True)
    weights = [p.detach().cpu().clone() for p in net.parameters()]
    state = dict(config=dataclasses.asdict(cfg), net={k: v.cpu() for k, v in net.state_dict().items()},
                 emas=[weights, [w.clone() for w in weights]], distill=dict(
                     agreement=agree, rows=int(len(obs)), entries=int(len(sc)),
                     epochs=args.epochs, lr=args.lr, teacher=args.teacher,
                     trained="all but the value module" if args.ckpt == "scratch" else "run_head"))
    torch.save(state, args.out)
    print(f"-> {args.out}")


@torch.no_grad()
def evaluate(net, rows, sc, lp, batch, ent_of):
    """Held-out top-1 agreement with the prior per kind (policy renormalised over the options),
    and with the teacher on run rows without an entry."""
    hit = collections.defaultdict(list)
    for a in range(0, len(rows), 1024):
        r = rows[a:a + 1024]
        _, teach, stud = batch(r)
        ls = torch.log_softmax(stud, -1)
        pos = [(k, j) for k, i in enumerate(r) for j in ent_of.get(int(i), ())]
        if pos:
            k = torch.tensor([p[0] for p in pos], device=ls.device)
            j = torch.tensor([p[1] for p in pos], device=ls.device)
            q = option_logq(ls[k], torch.from_numpy(sc.a2o[j.cpu().numpy()]).to(ls.device))
            same = (q.argmax(1) == lp[j].argmax(1)).cpu().numpy()
            for (_, jj), s in zip(pos, same):
                hit[KINDS[sc.kind[jj]]].append(bool(s))
        none = [k for k, i in enumerate(r) if int(i) not in ent_of]
        if none:
            nk = torch.tensor(none, device=stud.device)
            same = (stud[nk].argmax(1) == teach[nk].argmax(1)).cpu().numpy()
            hit["teacher(other run rows)"] += same.tolist()
    return {k: [round(float(np.mean(v)), 3), len(v)] for k, v in hit.items()}


def cmd_probe(args):
    """Greedy DEV runs of a checkpoint: the decision rates (Smith overall and at HP >= 70% of rest
    visits offering both, card-reward skips, shop removals per run, gold on entering a shop,
    removal affordable there) and floor / wins."""
    import e2e_eval as V
    torch.set_num_threads(2)
    device = torch.device(args.device)
    net = _policy(args, device)
    first, _ = V.PANELS["DEV-1000"]
    seeds = list(range(first, first + args.runs))
    c = collections.Counter()
    shop_gold, removals, floors, wins = [], collections.Counter(), [], 0

    def on_row(i, o, m, combat, a, entry):
        if combat:
            return
        if entry:
            g = int(o[12]) | int(o[13]) << 8
            shop_gold.append((g, bool(legal_bits(m[None])[0][P.REMOVE_CARD])))
        for k, opts, a2o in row_entries(o, legal_bits(m[None])[0], entry):
            ch = int(a2o[a])
            if ch < 0:
                continue
            kind, t = KINDS[k], opts[ch][0]
            if kind == "rest" and {D.T_SMITH, D.T_HEAL} <= {x[0] for x in opts}:
                hp = (int(o[8]) | int(o[9]) << 8) / max(1, int(o[10]) | int(o[11]) << 8)
                band = "hi" if hp >= 0.7 else "lo"
                c[f"rest_{band}"] += 1
                c[f"smith_{band}"] += t == D.T_SMITH
            elif kind == "card_reward":
                c["reward"] += 1
                c["skip"] += t == D.T_SKIP
            elif kind == "shop_remove":
                removals[i] += 1
            elif kind == "shop_removal":
                c["removal_visits"] += 1
                c["removal_used"] += t == D.T_REMOVE

    def on_end(i, env):
        nonlocal wins
        floors.append(int(env.floor[i]))
        wins += int(env.won[i])

    t0 = time.time()
    for b in range(0, len(seeds), args.batch):
        part = seeds[b:b + args.batch]
        removals.clear()
        play(net, part, device, args.threads, None, on_row, on_end)
        c["removals"] += sum(removals.values())
        print(f"{b + len(part)} runs ({time.time() - t0:.0f}s)", flush=True)
    n = len(seeds)
    g = np.asarray([x[0] for x in shop_gold]) if shop_gold else np.zeros(1)
    res = dict(ckpt=str(args.ckpt), weights=args.weights, runs=n,
               smith=(c["smith_hi"] + c["smith_lo"]) / max(1, c["rest_hi"] + c["rest_lo"]),
               smith_hi=c["smith_hi"] / max(1, c["rest_hi"]), rest_visits=c["rest_hi"] + c["rest_lo"],
               rest_hi=c["rest_hi"], skip=c["skip"] / max(1, c["reward"]), rewards=c["reward"],
               removals_per_run=c["removals"] / n, shops=len(shop_gold),
               shop_gold_mean=float(g.mean()), shop_gold_median=float(np.median(g)),
               removal_affordable=float(np.mean([x[1] for x in shop_gold])) if shop_gold else None,
               removal_first_choice=c["removal_used"] / max(1, c["removal_visits"]),
               floor=float(np.mean(floors)), wins=wins)
    print(json.dumps(res))
    if args.out:
        pathlib.Path(args.out).write_text(json.dumps(res, indent=1))


def main(argv=None):
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    dev = "cuda" if torch.cuda.is_available() else "cpu"
    for name in ("collect", "distill", "probe"):
        s = sub.add_parser(name)
        s.add_argument("ckpt")
        s.add_argument("--weights", default="ema0.999")
        s.add_argument("--device", default=dev)
        s.add_argument("--prior", default=str(P.DATA / "prior.pt"))
        s.add_argument("--out")
        s.add_argument("--threads", type=int, default=12)
        s.add_argument("--batch", type=int, default=500)
        if name == "collect":
            s.add_argument("--runs", type=int, default=2000)
            s.add_argument("--modes", default="greedy,prior")
        if name == "probe":
            s.add_argument("--runs", type=int, default=1000)
        if name == "distill":
            s.add_argument("--data", required=True)
            s.add_argument("--epochs", type=int, default=6)
            s.add_argument("--lr", type=float, default=1e-3)
            s.add_argument("--lr-decay", type=float, default=0.6)
            s.add_argument("--teacher", type=float, default=1.0)
            s.add_argument("--set", action="append", default=[],
                           help="scratch: key=json Config overrides (the lane's net)")
    args = ap.parse_args(argv)
    if args.cmd == "distill":
        args.batch = 1024 if args.batch == 500 else args.batch
    {"collect": cmd_collect, "distill": cmd_distill, "probe": cmd_probe}[args.cmd](args)


if __name__ == "__main__":
    sys.exit(main())
