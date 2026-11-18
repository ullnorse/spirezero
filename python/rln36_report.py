"""RLN36 tables (`docs/research/rln36-boss-wall.md`) from `rln36_boss.py` outputs.

    python python/rln36_report.py entry H...          # entry state at each boss, won vs lost
    python python/rln36_report.py play H... [--act3 F] # per-turn patterns of boss fights
    python python/rln36_report.py cards H...          # deck cards that move boss win rate
    python python/rln36_report.py cf F...             # counterfactual tables (act-2 cf, act-3)
"""

import argparse
import collections
import pathlib
import sys

import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import rln19_act2 as R  # noqa: E402

F = {k: i for i, k in enumerate(("turn", "hp", "block", "energy", "incoming", "osty", "osty_hp",
                                 "ehp", "emax", "belt", "kind", "what"))}
ORDER = ("CEREMONIAL_BEAST_BOSS", "LAGAVULIN_MATRIARCH_BOSS", "SOUL_FYSH_BOSS", "THE_KIN_BOSS",
         "VANTOM_BOSS", "WATERFALL_GIANT_BOSS", "KAISER_CRAB_BOSS", "KNOWLEDGE_DEMON_BOSS",
         "THE_INSATIABLE_BOSS", "AEONGLASS_BOSS", "QUEEN_BOSS", "TEST_SUBJECT_BOSS",
         "DOORMAKER_BOSS")


def short(b):
    return b.replace("_BOSS", "").replace("_", " ").title()


def load(paths):
    out = []
    for p in paths:
        _, rows = R.read_jsonl(p)
        out += rows
    return out


def bosses_in(fights):
    seen = list(dict.fromkeys(ORDER))
    seen += sorted({f["boss"] for f in fights} - set(seen))
    return [b for b in seen if any(f["boss"] == b for f in fights)]


def won(f):
    return f["end"] == "won"


def turns(f):
    """Per turn: start HP, start block, incoming, enemy HP at start, cards, potions, energy left
    at a voluntary end turn, max block reached."""
    by = collections.OrderedDict()
    for r in f["rows"]:
        by.setdefault(r[0], []).append(r)
    ts = list(by.items())
    out = []
    for k, (t, rows) in enumerate(ts):
        nxt = ts[k + 1][1][0] if k + 1 < len(ts) else f["final"]
        hp0, ehp0 = rows[0][1], rows[0][7]
        out.append(dict(
            turn=t, hp=hp0, inc=rows[0][4], lost=max(0, hp0 - nxt[1]), dealt=min(max(0, ehp0 - nxt[7]), rows[0][8]),
            block=max(r[2] for r in rows), cards=sum(r[10] == "card" for r in rows),
            pots=sum(r[10] == "potion" for r in rows), energy0=rows[0][3],
            eleft=next((r[3] for r in rows if r[10] == "end"), None), osty=rows[0][6]))
    return out


def stats(f):
    ts = turns(f)
    e = f["entry"]
    inc = [t["inc"] for t in ts]
    big = [t for t in ts if t["inc"] >= 20]
    pots_used = sum(t["pots"] for t in ts)
    return dict(
        turns=len(ts), dealt=np.mean([t["dealt"] for t in ts]),
        lost=np.mean([t["lost"] for t in ts]), inc=np.mean(inc),
        cover=sum(min(t["block"], t["inc"]) for t in ts) / max(1, sum(inc)),
        bigcover=(sum(min(t["block"], t["inc"]) for t in big) / max(1, sum(t["inc"] for t in big))
                  if big else np.nan),
        eleft=np.mean([t["eleft"] for t in ts if t["eleft"] is not None] or [np.nan]),
        vol=np.mean([t["eleft"] is not None for t in ts]),
        cards=np.mean([t["cards"] for t in ts]), pots=pots_used,
        belt_end=f["final"][9], boss_left=f["final"][7] / max(1, f["final"][8]) if f["final"][8] else 0,
        hp_frac=e["hp"] / max(1, e["max_hp"]), entry_pots=e["potions"])


def cmd_entry(args):
    fights = load(args.files)
    print("| act | boss | fights | win % | outcome | HP % | HP | deck | atk | skill | power | curse "
          "| upgr | relics | potions | gold |")
    print("|---|---|---:|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
    for b in bosses_in(fights):
        fs = [f for f in fights if f["boss"] == b]
        w = sum(map(won, fs))
        for name, sel in (("won", [f for f in fs if won(f)]), ("lost", [f for f in fs if not won(f)])):
            if not sel:
                continue
            m = lambda k: np.mean([f["entry"][k] for f in sel])  # noqa: E731
            hpf = np.mean([f["entry"]["hp"] / f["entry"]["max_hp"] for f in sel]) * 100
            print(f"| {fs[0]['act']} | {short(b)} | {len(fs)} | {100 * w / len(fs):.0f} | {name} {len(sel)} "
                  f"| {hpf:.0f} | {m('hp'):.0f} | {m('deck'):.1f} | {m('attacks'):.1f} | {m('skills'):.1f} "
                  f"| {m('powers'):.1f} | {m('curses'):.1f} | {m('upgraded'):.1f} | {m('relics'):.1f} "
                  f"| {m('potions'):.1f} | {m('gold'):.0f} |")
    print()
    print("Win % by entry HP %:\n\n| boss | >= 95 | 80-95 | 60-80 | < 60 |\n|---|---:|---:|---:|---:|")
    for b in bosses_in(fights):
        fs = [f for f in fights if f["boss"] == b]
        cells = []
        for lo, hi in ((0.95, 9), (0.8, 0.95), (0.6, 0.8), (-1, 0.6)):
            s = [f for f in fs if lo <= f["entry"]["hp"] / f["entry"]["max_hp"] < hi]
            cells.append(f"{100 * sum(map(won, s)) / len(s):.0f}% of {len(s)}" if s else "-")
        print(f"| {short(b)} | " + " | ".join(cells) + " |")


def cmd_play(args):
    fights = load(args.files)
    print("| boss | outcome | n | turns | dealt/turn | HP lost/turn | incoming/turn | block/incoming "
          "| same, incoming>=20 | voluntary end turn | energy left then | cards/turn | potions used | belt at end "
          "| boss HP left |")
    print("|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
    for b in bosses_in(fights):
        fs = [f for f in fights if f["boss"] == b]
        for name, sel in (("won", [f for f in fs if won(f)]), ("lost", [f for f in fs if not won(f)])):
            if not sel:
                continue
            S = [stats(f) for f in sel]
            m = lambda k: np.nanmean([s[k] for s in S])  # noqa: E731
            print(f"| {short(b)} | {name} | {len(sel)} | {m('turns'):.1f} | {m('dealt'):.1f} | "
                  f"{m('lost'):.1f} | {m('inc'):.1f} | {m('cover'):.2f} | {m('bigcover'):.2f} | "
                  f"{100 * m('vol'):.0f}% | {m('eleft'):.2f} | {m('cards'):.2f} | {m('pots'):.2f} | {m('belt_end'):.2f} | "
                  f"{100 * m('boss_left'):.0f}% |")
    print("\nLosses: how they end\n")
    print("| boss | lost | died holding potions | boss HP left <= 25% | > 50% | turn of death (median) "
          "| died on a turn with incoming >= HP+block |")
    print("|---|---:|---:|---:|---:|---:|---:|")
    for b in bosses_in(fights):
        sel = [f for f in fights if f["boss"] == b and not won(f) and f["end"] == "died"]
        if not sel:
            continue
        S = [stats(f) for f in sel]
        hold = sum(s["belt_end"] > 0 for s in S)
        low = sum(s["boss_left"] <= 0.25 for s in S)
        high = sum(s["boss_left"] > 0.5 for s in S)
        tod = np.median([s["turns"] for s in S])
        lethal = 0
        for f in sel:
            t = turns(f)[-1]
            lethal += t["inc"] >= t["hp"]
        print(f"| {short(b)} | {len(sel)} | {hold} | {low} | {high} | {tod:.0f} | {lethal} |")


def cmd_cards(args):
    fights = load(args.files)
    for b in bosses_in(fights):
        fs = [f for f in fights if f["boss"] == b]
        if len(fs) < 40:
            continue
        base = np.mean(list(map(won, fs)))
        cnt = collections.Counter()
        wins = collections.Counter()
        for f in fs:
            for c in set(x.rstrip("+") for x in f["entry"]["cards"]):
                cnt[c] += 1
                wins[c] += won(f)
        rows = [(100 * (wins[c] / n - (sum(map(won, fs)) - wins[c]) / max(1, len(fs) - n)), c, n)
                for c, n in cnt.items() if 0.1 * len(fs) <= n <= 0.9 * len(fs)]
        rows.sort()
        top = rows[:4] + rows[-4:]
        print(f"{short(b)} (n {len(fs)}, win {100 * base:.0f}%): " +
              ", ".join(f"{c} {d:+.0f}pp ({n})" for d, c, n in top))


def cmd_cf(args):
    import e2e_eval as EV

    for p in args.files:
        meta, rs = R.read_jsonl(p)
        if meta.get("kind") == "rln36-act3":
            rs = [r for r in meta["cfs"] if "greedy" in r["ends"] and len(r["ends"]) > 1]
        print(f"\n{p}: {meta.get('kind')}  samples {meta.get('samples')}  k {meta.get('k')} n {meta.get('n')}\n")
        cols = ("greedy", "potion", "any sample", "mean sample", "look")
        print("| boss | fights | " + " | ".join(cols) + " |\n|---|---:|" + "---:|" * len(cols))
        for b in bosses_in([dict(boss=r["boss"]) for r in rs]) + ["ALL"]:
            sel = [r for r in rs if b == "ALL" or r["boss"] == b]
            n = len(sel)
            cells = []
            for c in cols:
                if c == "any sample":
                    k = sum(any(v == "won" for kk, v in r["ends"].items() if kk[0] == "s" and kk[1:].isdigit())
                            for r in sel)
                elif c == "mean sample":
                    vals = [np.mean([v == "won" for kk, v in r["ends"].items() if kk[0] == "s" and kk[1:].isdigit()])
                            for r in sel if any(kk[0] == "s" and kk[1:].isdigit() for kk in r["ends"])]
                    cells.append(f"{100 * np.mean(vals):.1f}%" if vals else "-")
                    continue
                else:
                    have = [r for r in sel if c in r["ends"] or (c == "greedy" and "greedy" not in r["ends"])]
                    if c not in sel[0]["ends"] and c != "greedy":
                        cells.append("-")
                        continue
                    k = sum(r["ends"].get(c) == "won" for r in sel)
                lo, hi = EV.wilson(k, n)
                cells.append(f"{k} ({100 * k / n:.0f}%, {100 * lo:.0f}-{100 * hi:.0f})")
            print(f"| {short(b) if b != 'ALL' else 'All'} | {n} | " + " | ".join(cells) + " |")
        if "look" in rs[0]["ends"]:
            g = [r["ends"]["greedy"] == "won" for r in rs]
            lk = [r["ends"]["look"] == "won" for r in rs]
            both = sum(a and b for a, b in zip(g, lk))
            go = sum(a and not b for a, b in zip(g, lk))
            lo_ = sum(b and not a for a, b in zip(g, lk))
            nei = len(rs) - both - go - lo_
            _, lo, hi = EV.newcombe_paired(both, lo_, go, nei)
            print(f"\nlookahead vs greedy, paired: both {both}, greedy-only {go}, look-only {lo_}, "
                  f"neither {nei}; diff {100 * (lo_ - go) / len(rs):+.1f} pp ({100 * lo:+.1f} to {100 * hi:+.1f})")


def cmd_runs(args):
    """Override runs (`harvest --smith/--remove`), the first file the paired base."""
    import e2e_eval as EV

    base = None
    print("| variant | runs | overrides | mean floor | act-1 boss reached | act-1 boss won | act-2 boss "
          "reached | act-2 boss won | act-3 boss reached | floor vs base, paired (95%) |")
    print("|---|---:|---|---:|---:|---:|---:|---:|---:|---|")
    for p in args.files:
        meta, fights = R.read_jsonl(p)
        floors = {r["seed"]: r["floor"] for r in meta["runs"]}
        cell = {}
        for a in (1, 2, 3):
            fa = [f for f in fights if f["act"] == a]
            cell[a] = (len({f["seed"] for f in fa}), sum(won(f) for f in fa))
        name = pathlib.Path(p).name.split(".")[0]
        if base is None:
            base, d = floors, ""
        else:
            seeds = sorted(set(base) & set(floors))
            m, lo, hi = EV.paired_mean([floors[s] for s in seeds], [base[s] for s in seeds])
            d = f"{m:+.2f} ({lo:+.2f} to {hi:+.2f})"
        n = len(floors)
        print(f"| {name} | {n} | {meta.get('overrides', {})} | {np.mean(list(floors.values())):.2f} | "
              f"{cell[1][0]} | {cell[1][1]} ({100 * cell[1][1] / max(1, cell[1][0]):.0f}%) | {cell[2][0]} | "
              f"{cell[2][1]} ({100 * cell[2][1] / max(1, cell[2][0]):.0f}%) | {cell[3][0]} | {d} |")


def main():
    p = argparse.ArgumentParser()
    p.add_argument("cmd", choices=("entry", "play", "cards", "cf", "runs"))
    p.add_argument("files", nargs="+")
    a = p.parse_args()
    {"entry": cmd_entry, "play": cmd_play, "cards": cmd_cards, "cf": cmd_cf, "runs": cmd_runs}[a.cmd](a)


if __name__ == "__main__":
    main()
