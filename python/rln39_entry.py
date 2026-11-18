"""RLN39 §2/§4: how much the entry state of a boss fight explains (5-fold ridge-logistic AUC) and
the won-vs-lost deck comparison.

    python python/rln39_entry.py auc H.gz...      # AUC by feature set, per boss
    python python/rln39_entry.py deck H.gz... [--boss KAISER_CRAB_BOSS]   # card presence, won vs lost

Feature sets: `hp` (HP fraction), `deck counts` (RLN36's: gold, potions, relics, deck size, card types, upgrades, no HP), `state` (HP, max HP, gold, potions, relics, deck size, attacks,
skills, powers, curses, upgrades), `cards` (copy count of every card in >= 5% of the boss's decks,
plus the `state` set). `ceiling` is the losses removed if every fight won at the >= 95%-HP rate.
"""

import argparse
import collections
import pathlib
import sys

import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import rln19_act2 as R  # noqa: E402
import rln36_report as RP  # noqa: E402

STATE = ("hp", "max_hp", "gold", "potions", "relics", "deck", "attacks", "skills", "powers", "curses",
         "upgraded")


def fit(X, y, lam=1.0, iters=30):
    """Ridge logistic regression by Newton's method; returns the weights (bias last)."""
    X = np.hstack([X, np.ones((len(X), 1))])
    w = np.zeros(X.shape[1])
    reg = lam * np.eye(X.shape[1])
    reg[-1, -1] = 0
    for _ in range(iters):
        p = 1 / (1 + np.exp(-np.clip(X @ w, -30, 30)))
        g = X.T @ (p - y) + reg @ w
        H = (X * (p * (1 - p))[:, None]).T @ X + reg + 1e-6 * np.eye(len(w))
        w -= np.linalg.solve(H, g)
    return w


def auc(score, y):
    """Mann-Whitney AUC with ties averaged."""
    order = np.argsort(score, kind="mergesort")
    ranks = np.empty(len(score))
    s = score[order]
    i = 0
    while i < len(s):
        j = i
        while j + 1 < len(s) and s[j + 1] == s[i]:
            j += 1
        ranks[order[i:j + 1]] = (i + j) / 2 + 1
        i = j + 1
    pos = y == 1
    n1, n0 = pos.sum(), (~pos).sum()
    return float((ranks[pos].sum() - n1 * (n1 + 1) / 2) / (n1 * n0)) if n1 and n0 else float("nan")


def cv_auc(X, y, folds=5, seed=0):
    X = (X - X.mean(0)) / np.where(X.std(0) > 0, X.std(0), 1)
    idx = np.random.default_rng(seed).permutation(len(y))
    score = np.zeros(len(y))
    for k in range(folds):
        te = idx[k::folds]
        tr = np.setdiff1d(idx, te)
        w = fit(X[tr], y[tr])
        score[te] = np.hstack([X[te], np.ones((len(te), 1))]) @ w
    return auc(score, y)


def features(fights):
    state = np.array([[f["entry"][k] for k in STATE] for f in fights], dtype=float)
    hp = np.array([[f["entry"]["hp"] / max(1, f["entry"]["max_hp"])] for f in fights])
    cnt = collections.Counter(c for f in fights for c in set(f["entry"]["cards"]))
    keep = [c for c, n in cnt.items() if n >= 0.05 * len(fights)]
    cards = np.array([[f["entry"]["cards"].count(c) for c in keep] for f in fights], dtype=float)
    # RLN36's "deck counts": the state without HP and max HP.
    return hp, state[:, 2:], state, np.hstack([state, cards])


def cmd_auc(args):
    fights = RP.load(args.files)
    print("| boss | n | win | AUC HP % | AUC deck counts | AUC state | AUC state+cards | losses removed at >= 95%-HP win rate |"
          "\n|---|---:|---:|---:|---:|---:|---:|---:|")
    for b in RP.bosses_in(fights):
        fs = [f for f in fights if f["boss"] == b]
        if len(fs) < 80:
            continue
        y = np.array([RP.won(f) for f in fs], dtype=float)
        hp, counts, state, cards = features(fs)
        top = [f for f in fs if f["entry"]["hp"] >= 0.95 * f["entry"]["max_hp"]]
        rate = sum(map(RP.won, top)) / max(1, len(top))
        lost = len(fs) - y.sum()
        removed = max(0.0, (len(fs) * rate - y.sum()) / lost) if lost else 0
        print(f"| {RP.short(b)} | {len(fs)} | {100 * y.mean():.0f}% | {cv_auc(hp, y):.2f} "
              f"| {cv_auc(counts, y):.2f} | {cv_auc(state, y):.2f} | {cv_auc(cards, y):.2f} | <= {100 * removed:.0f}% |")


def cmd_deck(args):
    fights = [f for f in RP.load(args.files) if f["boss"] == args.boss]
    won = [f for f in fights if RP.won(f)]
    lost = [f for f in fights if not RP.won(f)]
    print(f"{args.boss}: won {len(won)}, lost {len(lost)}")
    print("| entry | won | lost |\n|---|---:|---:|")
    for k in ("hp", "max_hp", "deck", "attacks", "skills", "powers", "curses", "upgraded", "relics",
              "potions", "gold"):
        print(f"| {k} | {np.mean([f['entry'][k] for f in won]):.1f} | {np.mean([f['entry'][k] for f in lost]):.1f} |")
    cnt = collections.Counter(c.rstrip("+") for f in fights for c in set(f["entry"]["cards"]))
    rows = []
    for c, n in cnt.items():
        if not 0.1 * len(fights) <= n <= 0.9 * len(fights):
            continue
        has = [RP.won(f) for f in fights if c in {x.rstrip("+") for x in f["entry"]["cards"]}]
        no = [RP.won(f) for f in fights if c not in {x.rstrip("+") for x in f["entry"]["cards"]}]
        rows.append((np.mean(has) - np.mean(no), c, n))
    rows.sort()
    print("\ncards present vs absent (win rate pp, decks with the card): lowest 5, highest 8")
    print(", ".join(f"{c} {100 * d:+.0f} ({n})" for d, c, n in rows[:5] + rows[-8:]))


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    p.add_argument("cmd", choices=("auc", "deck"))
    p.add_argument("files", nargs="+")
    p.add_argument("--boss", default="KAISER_CRAB_BOSS")
    a = p.parse_args()
    {"auc": cmd_auc, "deck": cmd_deck}[a.cmd](a)


if __name__ == "__main__":
    main()
