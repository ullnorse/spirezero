"""Day 20, the capstone: train the slot net with PPO on real Act 1 fights for a time budget,
evaluate it by the protocol against random and cards first, and write the report. Reference
solution.

Run:  .venv/bin/python learn/rl/exercises/week4/day20_capstone.py [minutes] [out_dir]
      five minutes on an RTX 4080 Super is the intended budget; the report lands in out_dir.
"""

import json
import pathlib
import sys
import time

ROOT = pathlib.Path(__file__).resolve()
while not (ROOT / "Cargo.toml").exists():
    ROOT = ROOT.parent
sys.path[:0] = [str(ROOT / "python"), str(ROOT / "learn/rl/exercises")]

import numpy as np  # noqa: E402
import torch  # noqa: E402

from common import make_envs  # noqa: E402
from solutions.week2.day8_nets import DEVICE  # noqa: E402
from solutions.week2.day10_ppo import update  # noqa: E402
from solutions.week3.day11_experiment import COLUMNS, format_row  # noqa: E402
from solutions.week3.day13_eval import cards_first_policy, compare, evaluate, greedy, random_legal  # noqa: E402
from solutions.week3.day14_features import SlotNet, featurise14, rollout  # noqa: E402

DEFAULTS = dict(n_envs=256, T=128, lr=3e-4, epochs=4, minibatch=4096, ent_coef=0.01, clip=0.2, seed=0, eval_every=10,
                eval_fights=1024, threads=4)


def train_for(budget_s, out_dir, log=print, **overrides):
    """PPO with the slot net for `budget_s` seconds of wall time (the last iteration may run
    over). Settings are DEFAULTS updated by `overrides`. Every `eval_every` iterations and
    after the last, `eval_win` by day 13's `evaluate` with `greedy(net, featurise14)` over
    `eval_fights` fights, and `best.pt` (state_dict + the settings + it + eval_win) is saved
    when it improves. Rows as day 11's COLUMNS with `steps` the env steps so far; the history
    goes to `history.json` in `out_dir`. Returns (best net: a SlotNet loaded from best.pt,
    history)."""
    cfg = {**DEFAULTS, **overrides}
    out_dir = pathlib.Path(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    torch.manual_seed(cfg["seed"])
    envs = make_envs(cfg["n_envs"], seed=cfg["seed"], threads=cfg["threads"])
    net = SlotNet().to(DEVICE)
    opt = torch.optim.Adam(net.parameters(), lr=cfg["lr"], eps=1e-5)
    history, steps, best, start, it = [], 0, -1.0, time.perf_counter(), 0
    while True:
        t0 = time.perf_counter()
        batch = rollout(envs, net, cfg["T"], featurise14)
        stats = update(net, opt, batch, epochs=cfg["epochs"], minibatch=cfg["minibatch"], clip=cfg["clip"], ent_coef=cfg["ent_coef"])
        steps += cfg["T"] * cfg["n_envs"]
        last = time.perf_counter() - start >= budget_s
        row = {"it": it, "steps": steps, "win_rate": batch["wins"] / max(1, batch["episodes"]), "eval_win": None,
               "entropy": stats["entropy"], "clipfrac": stats["clipfrac"], "approx_kl": stats["approx_kl"],
               "value_loss": stats["value_loss"], "mean_length": cfg["T"] * cfg["n_envs"] / max(1, batch["episodes"]),
               "sps": cfg["T"] * cfg["n_envs"] / (time.perf_counter() - t0)}
        if (it + 1) % cfg["eval_every"] == 0 or last:
            row["eval_win"] = evaluate(greedy(net, featurise14), n_fights=cfg["eval_fights"], threads=cfg["threads"])["win_rate"]
            if row["eval_win"] > best:
                best = row["eval_win"]
                torch.save({"state_dict": net.state_dict(), "meta": {**cfg, "it": it, "eval_win": best}}, out_dir / "best.pt")
        history.append(row)
        if log:
            log(format_row(row))
        it += 1
        if last:
            break
    envs.close()
    (out_dir / "history.json").write_text(json.dumps(history, indent=1))
    best_net = SlotNet().to(DEVICE)
    best_net.load_state_dict(torch.load(out_dir / "best.pt", map_location=DEVICE)["state_dict"])
    best_net.eval()
    return best_net, history


def report(net, history, out_dir, n_fights=1024, threads=4):
    """Evaluates random, cards first and the net by day 13's protocol over `n_fights` each,
    writes `report.json` ({"policies": {name: evaluate result}, "steps", "iterations",
    "best_eval_win", "encounters": compare(cards first, net)'s rows}) and `report.md` (a
    table of the three policies: win rate ± se, mean length; then the per-encounter table of
    cards first against the net, sorted by gain) to `out_dir`. Returns the dict."""
    out_dir = pathlib.Path(out_dir)
    policies = {"random": evaluate(random_legal, n_fights, threads=threads),
                "cards first": evaluate(cards_first_policy, n_fights, threads=threads),
                "slot net (best)": evaluate(greedy(net, featurise14), n_fights, threads=threads)}
    diff, se, rows = compare(policies["cards first"], policies["slot net (best)"])
    evals = [h["eval_win"] for h in history if h["eval_win"] is not None]
    out = {"policies": policies, "steps": history[-1]["steps"], "iterations": len(history), "best_eval_win": max(evals),
           "encounters": rows, "gain_over_cards_first": [diff, se]}
    (out_dir / "report.json").write_text(json.dumps(out, indent=1))
    lines = ["# Capstone report", "", f"{len(history)} iterations, {history[-1]['steps']:,} env steps, best eval win {max(evals):.3f}.", "",
             "| policy | win rate | se | mean length |", "|---|---:|---:|---:|"]
    for name, r in policies.items():
        lines.append(f"| {name} | {r['win_rate']:.3f} | {r['se']:.3f} | {r['mean_length']:.1f} |")
    lines += ["", f"Slot net minus cards first: {diff:+.3f} ± {se:.3f}", "", "| encounter | cards first | slot net | fights |", "|---|---:|---:|---:|"]
    for name, pa, pb, na, nb in rows:
        lines.append(f"| {name} | {pa:.2f} | {pb:.2f} | {na}/{nb} |")
    (out_dir / "report.md").write_text("\n".join(lines) + "\n")
    return out


def main(argv):
    minutes = float(argv[1]) if len(argv) > 1 else 5.0
    out_dir = pathlib.Path(argv[2]) if len(argv) > 2 else ROOT / "target/learn/capstone"
    net, history = train_for(minutes * 60, out_dir)
    out = report(net, history, out_dir)
    for name, r in out["policies"].items():
        print(f"{name:16s} win {r['win_rate']:.3f} ± {r['se']:.3f}  length {r['mean_length']:.1f}")
    print(f"report: {out_dir / 'report.md'}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
