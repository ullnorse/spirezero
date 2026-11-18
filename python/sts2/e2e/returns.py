"""Two-chain returns with run-row hold-back, and per-decision-type advantage moments (RLN15).

A port of spire-mentor's e2e algorithm core (`sts2-pycli/python/ppo/e2e/algo.py`: `_scan`
:207, `compute_returns` :304, `HoldBack.carry` :548 and `.resume` :582, `AdvNorm` :619); the
learner turns them on with `Config.returns = "two_chain"` and `Config.adv_norm = "per_type"`.

**Table.** One row per stored decision, as numpy columns: ``run_uid``, ``t`` (order within the
run), ``is_combat``, ``dtype``, ``reward`` (all reward from the row to its successor),
``done``, ``bootstrap``, ``value`` and ``ref`` (the row's index into the learner's inputs; -1
on bootstrap rows), plus ``age``, ``waited`` and ``carried`` on held rows. A run still in
progress at the end of the rollout ends in a *bootstrap row*: a value-only row holding the
value of the next state. A practice segment that ends by truncation ends in a bootstrap row
with ``done`` set (the segment is over; its value is the bootstrap). A run that ended has a
non-bootstrap ``done`` row last (value 0 after it).

**Returns (γ = 1).** Two chains over each run's rows:

- *combat chain*, by row adjacency: ``δ_t = r_t + V(succ) - V(t)`` and
  ``A_t = δ_t + λ_c · 1[succ is a non-bootstrap combat row] · A_succ``. A fight's chain stops
  at the fight's end and hands its value to the next run row (``V(succ)``).
- *run chain*, over the run rows alone: ``δ_i = R(i→j) + V(j) - V(i)`` and
  ``A_i = δ_i + λ_r · A_j``, with j the next run row and R(i→j) the sum of every row's reward
  from i up to j. Without a next run row, j is the end: the bootstrap row (cut) or the
  terminal (value 0).

Combat rows train in the iteration that stored them. Run rows are *held back*: they wait
until their run ends or until they are ``hold_iters`` (4) iterations old, then train with the
chain cut at the current bootstrap row. While they wait, the rewards of the rows that trained
and left are folded into them (`carry`), so R stays exact. A resume drops every held row
(theirs: a restart loses its open runs; ours replays them, but the held rows' inputs are not
saved).
"""

import math

import numpy as np

LAM_C, LAM_R, HOLD_ITERS = 0.95, 0.97, 4
ADV_MIN_ROWS = 64
N_TYPES = 64

# Decision types (spire-mentor `contracts.py:822`, the same split on our phases): 1 combat
# turn, 2 combat choice; outside combat 8 + the phase code (`run_obs.rs` `phase_code`: 0
# ancient (Neow), 1 event, 2 map, 4 rewards, 6 bundles, 7 rest, 8 treasure, 9 shop); a deck
# selection (phase 5) is 32 + its purpose (`select_code`: upgrade, smith, remove, enchant,
# transform...).
DT_COMBAT_TURN, DT_COMBAT_CHOICE = 1, 2
_PHASE_AT, _PHASE_COMBAT, _PHASE_SELECT = 3, 3, 5
_PICK_BASE, _PICKS = 129, 10


def decision_types(obs, mask_bytes, select_at):
    """uint8 decision type per row, from the obs bytes (n, OBS_BYTES) and the mask bytes."""
    phase = obs[:, _PHASE_AT].astype(np.int64)
    bits = np.unpackbits(mask_bytes[:, _PICK_BASE // 8:(_PICK_BASE + _PICKS) // 8 + 1],
                         axis=1, bitorder="little")
    lo = _PICK_BASE % 8
    choice = bits[:, lo:lo + _PICKS].any(1)
    out = 8 + phase
    out = np.where(phase == _PHASE_SELECT, 32 + obs[:, select_at].astype(np.int64), out)
    combat = phase == _PHASE_COMBAT
    out = np.where(combat, np.where(choice, DT_COMBAT_CHOICE, DT_COMBAT_TURN), out)
    return np.clip(out, 0, N_TYPES - 1).astype(np.uint8)


def scan(delta, nxt, mult):
    """``A[i] = delta[i] + mult[i] · A[nxt[i]]`` over a forest of chains (``nxt = -1`` ends
    one), by pointer jumping (theirs, algo.py:207)."""
    s = np.asarray(delta, np.float64).copy()
    j = np.asarray(nxt, np.int64).copy()
    p = np.where(j >= 0, np.asarray(mult, np.float64), 0.0)
    act = np.flatnonzero(j >= 0)
    while act.size:
        jj = j[act]
        s[act] = s[act] + p[act] * s[jj]
        p[act] = p[act] * p[jj]
        j[act] = j[jj]
        act = act[j[act] >= 0]
    return s


def _col(table, name, dt, default=None):
    if name in table:
        return np.asarray(table[name]).astype(dt, copy=False)
    if default is None:
        raise KeyError(f"table has no column {name!r}")
    return np.full(len(np.asarray(table["run_uid"])), default, dt)


def compute_returns(table, *, lam_c=LAM_C, lam_r=LAM_R, hold_iters=HOLD_ITERS):
    """Two-chain GAE (γ = 1) over one iteration's table (theirs, algo.py:304-454).

    Returns a dict of row-aligned arrays: ``adv``, ``target`` (NaN where nothing trains),
    ``trained``, ``held`` (`carry` keeps these), ``dormant``, ``overdue``, ``boot_w`` (weight
    of the cut value in the target) and ``order`` (the (run_uid, t) sort)."""
    run = _col(table, "run_uid", np.int64)
    n = len(run)
    t = _col(table, "t", np.int64)
    comb = _col(table, "is_combat", bool)
    rew = _col(table, "reward", np.float64)
    done = _col(table, "done", bool)
    boot = _col(table, "bootstrap", bool)
    age = _col(table, "age", np.int64, default=0)
    carried = _col(table, "carried", bool, default=False)
    overdue = _col(table, "waited", np.int64, default=0) > hold_iters + 2
    v_in = _col(table, "value", np.float64)
    if n == 0:
        e, b = np.zeros(0), np.zeros(0, bool)
        return dict(adv=e, target=e, trained=b, held=b, dormant=b, overdue=b, boot_w=e,
                    order=np.zeros(0, np.int64))
    order = np.lexsort((t, run))
    r_, t_, c_, w_, d_, b_, g_, k_, v_ = (
        x[order] for x in (run, t, comb, rew, done, boot, age, carried, v_in))
    pos = np.arange(n)
    same = np.zeros(n, bool)  # same[p]: row p + 1 belongs to the same run
    same[:-1] = r_[1:] == r_[:-1]
    dup = same[:-1] & (t_[1:] <= t_[:-1])
    if dup.any():
        p = int(np.flatnonzero(dup)[0])
        raise ValueError(f"run {r_[p]}: t {t_[p]} appears twice")
    last = ~same
    first = np.ones(n, bool)
    first[1:] = ~same[:-1]
    starts, ends = np.flatnonzero(first), np.flatnonzero(last)
    rid = np.cumsum(first) - 1

    def bad(mask, what):
        if mask.any():
            p = int(np.flatnonzero(mask)[0])
            raise ValueError(f"run {r_[p]}, t {t_[p]}: {what}")

    bad(b_ & ~last, "a bootstrap row must be the last row of its run")
    bad(d_ & ~b_ & ~last, "a done row must be the last row of its run")
    bad(k_ & (c_ | b_), "carried rows must be run rows")
    end_boot = b_[ends]
    end_term = d_[ends] & ~b_[ends]
    open_run = ~end_boot & ~end_term
    has_fresh = np.logical_or.reduceat(~k_, starts)
    bad((open_run & has_fresh)[rid] & last, "new rows but neither a terminal nor a bootstrap row")
    dorm = open_run[rid]
    ended = (end_term | (end_boot & d_[ends]))[rid]

    cn = c_ & ~b_
    rn = ~c_ & ~b_ & ~dorm
    bad(~dorm & ~np.isfinite(v_), "value is not finite")
    bad((cn | rn) & ~np.isfinite(w_), "reward is not finite")

    succ = np.where(same, pos + 1, -1)
    has_s = succ >= 0
    vnext = np.zeros(n)
    vnext[has_s] = v_[succ[has_s]]
    delta = np.zeros(n)
    nxt = np.full(n, -1, np.int64)
    mult = np.zeros(n)

    # Combat chain: continue only into a non-bootstrap combat successor.
    delta[cn] = w_[cn] + vnext[cn] - v_[cn]
    ci = np.flatnonzero(cn & has_s)
    cs = succ[ci]
    ok = c_[cs] & ~b_[cs]
    nxt[ci[ok]] = cs[ok]
    mult[cn] = lam_c

    # Run chain over each run's run rows; R(i→j) by segment sums.
    rp = np.flatnonzero(rn)
    nq = np.full(rp.size, -1, np.int64)
    if rp.size > 1:
        same_r = rid[rp[1:]] == rid[rp[:-1]]
        nq[:-1][same_r] = rp[1:][same_r]
    r_eff = np.where(b_, 0.0, w_)
    seg = np.unique(np.concatenate([rp, starts, np.flatnonzero(b_)]))
    seg_sum = np.add.reduceat(r_eff, seg)
    big_r = seg_sum[np.searchsorted(seg, rp)]
    hq = nq >= 0
    vend = np.zeros(rp.size)
    vend[hq] = v_[nq[hq]]
    e_end = ends[rid[rp[~hq]]]
    vend[~hq] = np.where(b_[e_end], v_[e_end], 0.0)
    delta[rp] = big_r + vend - v_[rp]
    nxt[rp] = nq
    mult[rp] = lam_r

    adv_s = scan(delta, nxt, mult)

    cut_end = np.zeros(n)
    ce = np.flatnonzero(cn & (nxt < 0) & has_s)
    cut_end[ce] = b_[succ[ce]]
    cut_end[rp[~hq]] = b_[e_end]
    bw_s = scan(cut_end, nxt, mult)

    release = ended | (g_ >= hold_iters)
    trained = cn | (rn & release)
    held = (rn & ~release) | dorm
    adv_s = np.where(trained, adv_s, np.nan)
    bw_s = np.where(trained, bw_s, np.nan)

    def back(x):
        out = np.empty_like(x)
        out[order] = x
        return out

    return dict(adv=back(adv_s), target=back(adv_s + v_), trained=back(trained),
                held=back(held), dormant=back(dorm), overdue=overdue, boot_w=back(bw_s),
                order=order)


def carry(table, ret):
    """The held rows for the next iteration (theirs, `HoldBack.carry`, algo.py:548): each
    kept row's reward becomes its own plus that of every row after it up to the next kept row
    or the cut; ``age`` grows by 1 except on dormant rows; ``waited`` grows on all."""
    keep = np.asarray(ret["held"], bool)
    if not keep.any():
        return {}
    order = ret["order"]
    run_s = _col(table, "run_uid", np.int64)[order]
    boot_s = _col(table, "bootstrap", bool)[order]
    comb_s = _col(table, "is_combat", bool)[order]
    rew_s = _col(table, "reward", np.float64)[order]
    keep_s = keep[order]
    n = len(order)
    first = np.ones(n, bool)
    first[1:] = run_s[1:] != run_s[:-1]
    rp = np.flatnonzero(~comb_s & ~boot_s)
    if rp.size > 1:
        same = run_s[rp[1:]] == run_s[rp[:-1]]
        if np.any(same & keep_s[rp[:-1]] & ~keep_s[rp[1:]]):
            raise ValueError("a released run row follows a held one in the same run")
    kp = np.flatnonzero(keep_s)
    seg = np.unique(np.concatenate([kp, np.flatnonzero(first), np.flatnonzero(boot_s)]))
    sums = np.add.reduceat(np.where(boot_s, 0.0, rew_s), seg)
    folded = sums[np.searchsorted(seg, kp)]
    idx = order[kp]
    new = {k: np.asarray(v)[idx] for k, v in table.items()}
    new["reward"] = folded
    age = _col(table, "age", np.int64, default=0)[idx]
    new["age"] = age + np.where(np.asarray(ret["dormant"], bool)[idx], 0, 1)
    new["waited"] = _col(table, "waited", np.int64, default=0)[idx] + 1
    new["carried"] = np.ones(len(idx), bool)
    return new


def join(held, fresh):
    """This iteration's table: the held rows, then the fresh rows (age 0)."""
    f = {k: np.asarray(v) for k, v in fresh.items()}
    n = len(f["run_uid"])
    f["reward"] = f["reward"].astype(np.float64)
    f["age"] = np.zeros(n, np.int64)
    f["waited"] = np.zeros(n, np.int64)
    f["carried"] = np.zeros(n, bool)
    if not held:
        return f
    if set(held) != set(f):
        raise ValueError(f"fresh columns {sorted(f)} differ from the held rows' {sorted(held)}")
    return {k: np.concatenate([held[k], f[k]]) for k in f}


def _pooled(a):
    m = float(np.mean(a))
    return m, math.sqrt(max(1e-6, float(np.mean(a * a)) - m * m))


class AdvNorm:
    """Per decision type, exponentially weighted advantage moments (theirs, algo.py:619-724).

    A type's first ``min_rows`` (64) rows set its moments to their plain means of a and a²;
    after that each batch of k rows keeps ``(1 - decay)^k`` of the old ones (decay 5e-4 per
    row). std = sqrt(max(1e-6, E[a²] - E[a]²)). A type that has seen ``min_rows`` rows (this
    batch's included) uses its own moments; until then its rows use the pooled moments of the
    batch's rows of their family (combat or not), or of the whole batch when that family has
    fewer than ``min_rows`` rows too. The moments update before the batch is normalised."""

    def __init__(self, n_types=N_TYPES, decay=5e-4, min_rows=ADV_MIN_ROWS):
        self.decay, self.min_rows = float(decay), int(min_rows)
        self.mean = np.zeros(n_types)
        self.sq = np.ones(n_types)
        self.count = np.zeros(n_types, np.int64)

    def update(self, adv, dtype):
        a = np.asarray(adv, np.float64)
        d = np.asarray(dtype, np.int64)
        n_t = len(self.mean)
        if not np.all(np.isfinite(a)):
            raise ValueError("advantages must be finite")
        cnt = np.bincount(d, minlength=n_t)
        s1 = np.bincount(d, weights=a, minlength=n_t)
        s2 = np.bincount(d, weights=a * a, minlength=n_t)
        has = cnt > 0
        bm = np.where(has, s1 / np.maximum(cnt, 1), 0.0)
        bs = np.where(has, s2 / np.maximum(cnt, 1), 0.0)
        warm = has & (self.count >= self.min_rows)
        cold = has & ~warm
        w = self.count[cold] / (self.count[cold] + cnt[cold])
        self.mean[cold] = w * self.mean[cold] + (1.0 - w) * bm[cold]
        self.sq[cold] = w * self.sq[cold] + (1.0 - w) * bs[cold]
        keep = (1.0 - self.decay) ** cnt[warm]
        self.mean[warm] = keep * self.mean[warm] + (1.0 - keep) * bm[warm]
        self.sq[warm] = keep * self.sq[warm] + (1.0 - keep) * bs[warm]
        self.count += cnt

    def std(self):
        return np.sqrt(np.maximum(1e-6, self.sq - self.mean * self.mean))

    def normalize(self, adv, dtype, combat):
        a = np.asarray(adv, np.float64)
        d = np.asarray(dtype, np.int64)
        fam = np.asarray(combat, bool)
        mean, std = self.mean[d], self.std()[d]
        cold = self.count[d] < self.min_rows
        if cold.any():
            whole = _pooled(a)
            for f in (False, True):
                rows = cold & (fam == f)
                if rows.any():
                    in_f = fam == f
                    m, s = _pooled(a[in_f]) if int(in_f.sum()) >= self.min_rows else whole
                    mean[rows], std[rows] = m, s
        return (a - mean) / std

    def update_normalize(self, adv, dtype, combat):
        self.update(adv, dtype)
        return self.normalize(adv, dtype, combat)

    def to_dict(self):
        return dict(decay=self.decay, min_rows=self.min_rows, mean=self.mean.tolist(),
                    sq=self.sq.tolist(), count=self.count.tolist())

    @classmethod
    def from_dict(cls, d):
        a = cls(len(d["mean"]), float(d["decay"]), int(d["min_rows"]))
        a.mean[:] = d["mean"]
        a.sq[:] = d["sq"]
        a.count[:] = d["count"]
        return a
