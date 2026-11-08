"""VN1 diagnosis: how a combat value net scores the real bot's positions (`value-collect`
data), boss rows only: calibration and ranking by boss, turn and HP band; ranking within
(boss, turn) strata, which is how sibling leaves are compared; the net's price of player HP
against boss HP next to the outcomes'; and what it says late in fights the bot lost.

    .venv/bin/python python/vn1_diagnose.py --data VC --net RLB11/run2/last.pt [--first S]
"""

import argparse

import numpy as np
import torch

import vn1_common as vc
from sts2 import combat_net

HP_W = 0.3  # the search's weight of HP kept (`--value-hp 300`)


def leaf(p, q, hp_frac):
    """The search's leaf score per win: P(win) + 0.3 × min(HP kept, HP now)."""
    return p + HP_W * np.minimum(q, hp_frac)


def line(tag, m, p, win, s):
    if m.sum() == 0:
        return
    bce = -np.mean(win[m] * np.log(np.clip(p[m], 1e-6, 1)) + (1 - win[m]) * np.log(np.clip(1 - p[m], 1e-6, 1)))
    print(f"  {tag:<24} rows {int(m.sum()):6d} won {win[m].mean():.3f} pred {p[m].mean():.3f} "
          f"bce {bce:.4f} auc {vc.auc(s[m], win[m]):.3f}")


def strat_auc(s, win, keys):
    """Pairs-weighted AUC within each stratum of `keys`."""
    num = den = 0.0
    for k in np.unique(keys):
        m = keys == k
        pos, neg = win[m].sum(), (1 - win[m]).sum()
        if pos and neg:
            a = vc.auc(s[m], win[m])
            num += a * pos * neg
            den += pos * neg
    return num / den if den else float("nan")


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--data", required=True)
    ap.add_argument("--net", nargs="+", required=True)
    ap.add_argument("--first", type=int, default=0, help="only seeds from here")
    ap.add_argument("--last", type=int, default=1 << 62, help="only seeds before this")
    ap.add_argument("--device", default="cuda")
    a = ap.parse_args()
    torch.set_num_threads(2)
    dev = torch.device(a.device)
    raw, t, fight, enc, turn = vc.load(a.data)
    m = (t[:, 1] == 2) & (t[:, 0] >= a.first) & (t[:, 0] < a.last)
    raw, t, fight, enc, turn = raw[m], t[m], fight[m], enc[m], turn[m]
    win, hpk = vc.targets(t)
    names = vc.encounters()
    hp_frac = np.clip(raw[:, 0].astype(np.int64) + raw[:, 1].astype(np.int64) * 256, 0, None) / np.maximum(
        raw[:, 2].astype(np.int64) + raw[:, 3].astype(np.int64) * 256, 1)
    x = torch.from_numpy(raw).to(dev)
    print(f"{m.sum()} boss rows, {len(np.unique(fight))} fights, won {win.mean():.3f}")
    for path in a.net:
        net, _ = vc.load_net(path, dev)
        _, wl, hl = vc.heads(net, x)
        p, q = torch.sigmoid(wl).cpu().numpy(), torch.sigmoid(hl).cpu().numpy()
        s = leaf(p, q, hp_frac)
        print(f"== {path}")
        print(" by boss")
        for e in np.unique(enc):
            line(names.get(int(e), e), enc == e, p, win, s)
        print(" by turn")
        for lo, hi in ((1, 1), (2, 2), (3, 3), (4, 5), (6, 8), (9, 12), (13, 999)):
            line(f"turn {lo}-{hi}", (turn >= lo) & (turn <= hi), p, win, s)
        print(" by HP band at the turn start")
        for lo, hi in ((0, .25), (.25, .5), (.5, .75), (.75, 1.01)):
            line(f"hp {lo:.2f}-{hi:.2f}", (hp_frac >= lo) & (hp_frac < hi), p, win, s)
        print(" by predicted P(win) (reliability)")
        for lo, hi in ((0, .5), (.5, .8), (.8, .9), (.9, .95), (.95, .98), (.98, 1.01)):
            line(f"pred {lo:.2f}-{hi:.2f}", (p >= lo) & (p < hi), p, win, s)
        tb = np.minimum(turn, 12)
        print(f" ranking within (boss, turn): leaf auc {strat_auc(s, win, enc * 100 + tb):.3f}, "
              f"P auc {strat_auc(p, win, enc * 100 + tb):.3f}, "
              f"player-HP-only auc {strat_auc(hp_frac, win, enc * 100 + tb):.3f}")
        hk = win > 0
        print(f" HP kept on wins: actual {hpk[hk].mean():.3f} pred {q[hk].mean():.3f}; "
              f"corr {np.corrcoef(q[hk], hpk[hk])[0, 1]:.3f}")
        # Lost fights: the net's P(win) k turn starts before the end.
        ends = {}
        for i in range(len(fight)):
            ends[fight[i]] = i
        lost = [f for f, i in ends.items() if win[i] == 0]
        last = np.array([ends[f] for f in lost])
        print(f" lost fights {len(lost)}: P at the last turn start {p[last].mean():.3f}, "
              f"P > 0.9 there {np.mean(p[last] > 0.9):.2f}")
        for e in np.unique(enc[last]):
            mm = enc[last] == e
            print(f"   {names.get(int(e), e):<24} {int(mm.sum()):4d} lost, last-turn P {p[last][mm].mean():.3f}, "
                  f"turn {turn[last][mm].mean():.1f}, hp {hp_frac[last][mm].mean():.2f}")
        sens(net, raw, dev, s, hp_frac, enc, win, names, turn)


def creature_cols(i):
    at = combat_net.CREATURES_AT + i * combat_net.CREATURE_BYTES
    return at, at + 6, at + 8  # alive/side, hp, max hp


def sens(net, raw, dev, s, hp_frac, enc, win, names, turn):
    """The leaf score's change for 10% of max HP off the player, and 10% of max HP off every
    living enemy, against the outcomes' logistic fit on the same two features."""
    def score(r):
        x = torch.from_numpy(r).to(dev)
        _, wl, hl = vc.heads(net, x)
        p, q = torch.sigmoid(wl).cpu().numpy(), torch.sigmoid(hl).cpu().numpy()
        h = np.clip(r[:, 0].astype(np.int64) + r[:, 1].astype(np.int64) * 256, 0, None) / np.maximum(
            r[:, 2].astype(np.int64) + r[:, 3].astype(np.int64) * 256, 1)
        return leaf(p, q, h), p

    def i16(r, at):
        v = r[:, at].astype(np.int64) + r[:, at + 1].astype(np.int64) * 256
        return v - (v >= 32768) * 65536

    def put(r, at, v):
        v = np.clip(v, 0, 32767).astype(np.int64)
        r[:, at], r[:, at + 1] = v & 255, v >> 8

    a = raw.copy()
    mx = i16(a, 2)
    put(a, 0, np.maximum(i16(a, 0) - np.maximum(mx // 10, 1), 1))
    sa, pa = score(a)
    s0, p0 = score(raw)
    d_player, dp_player = s0 - sa, p0 - pa
    b = raw.copy()
    e_frac_num = np.zeros(len(raw))
    e_frac_den = np.zeros(len(raw))
    for i in range(combat_net.CREATURES):
        at, hp_at, max_at = creature_cols(i)
        enemy = (b[:, at] == 1) & (b[:, at + 1] == 1)
        hp, mxe = i16(b, hp_at), i16(b, max_at)
        e_frac_num += np.where(enemy, hp, 0)
        e_frac_den += np.where(enemy, mxe, 0)
        put(b, hp_at, np.where(enemy, np.maximum(hp - np.maximum(mxe // 10, 1), 1), hp))
    sb, pb = score(b)
    d_enemy, dp_enemy = sb - s0, pb - p0
    print(" sensitivity (leaf score per 10% max HP): ")
    for e in np.unique(enc):
        m = enc == e
        ef = e_frac_num[m] / np.maximum(e_frac_den[m], 1)
        # Outcome logistic fit on (player HP fraction, enemy HP fraction), via torch.
        tb = np.minimum(turn[m], 10)
        X = torch.tensor(np.concatenate([np.stack([hp_frac[m], ef], 1),
                                         np.eye(11)[tb][:, 1:]], 1), dtype=torch.float64)
        y = torch.tensor(win[m], dtype=torch.float64)
        w = torch.zeros(X.shape[1] + 1, dtype=torch.float64, requires_grad=True)
        opt = torch.optim.LBFGS([w], max_iter=200)

        def closure():
            opt.zero_grad()
            loss = torch.nn.functional.binary_cross_entropy_with_logits(X @ w[:-1] + w[-1], y)
            loss.backward()
            return loss
        opt.step(closure)
        with torch.no_grad():
            pr = torch.sigmoid(X @ w[:-1] + w[-1])
            g = (pr * (1 - pr)).mean().item()
            emp_p, emp_e = w[0].item() * g / 10, -w[1].item() * g / 10
        print(f"   {names.get(int(e), e):<24} net: player {d_player[m].mean():.4f} enemy {d_enemy[m].mean():.4f} "
              f"ratio {d_enemy[m].mean() / max(d_player[m].mean(), 1e-6):.2f}; P only {dp_player[m].mean():.4f} "
              f"{dp_enemy[m].mean():.4f} ratio {dp_enemy[m].mean() / max(dp_player[m].mean(), 1e-6):.2f} | outcomes: player {emp_p:.4f} "
              f"enemy {emp_e:.4f} ratio {emp_e / max(emp_p, 1e-6):.2f} (P only)")


if __name__ == "__main__":
    main()
