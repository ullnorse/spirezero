"""Day 11: the first experiment as a script. Baselines on a fixed seed, a log row per
iteration, checkpoints that reload to the same policy, and a run that keeps its history.
Reference solution.

Run:  .venv/bin/python learn/rl/exercises/week3/day11_experiment.py [iterations] [out_dir]
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
from solutions.week1.day2_observe import cards_first  # noqa: E402
from solutions.week2.day10_ppo import DEVICE, ActorCritic, evaluate, rollout, update  # noqa: E402

EVAL_SEED = 7
COLUMNS = ("it", "steps", "win_rate", "eval_win", "entropy", "clipfrac", "approx_kl", "value_loss", "mean_length", "sps")


def random_legal(legal, rng=np.random.default_rng(0)):
    """A uniform legal action per env (`policy(legal) -> actions`, day 2's shape)."""
    return np.where(legal, rng.random(legal.shape), -1.0).argmax(axis=1)


def measure(policy, n_envs=64, n_steps=1000, seed=EVAL_SEED):
    """Win rate and mean fight length (actions) of `policy(legal) -> actions` over the fights
    that end within `n_steps` steps of `make_envs(n_envs, seed)`. Returns (win_rate, fights,
    mean_length)."""
    envs = make_envs(n_envs, seed=seed)
    length = np.zeros(n_envs, dtype=np.int64)
    wins = fights = total = 0
    for _ in range(n_steps):
        envs.step(policy(envs.legal()))
        length += 1
        ended = envs.done.astype(bool)
        fights += int(ended.sum())
        wins += int(envs.won.sum())
        total += int(length[ended].sum())
        length[ended] = 0
    envs.close()
    return wins / max(1, fights), fights, total / max(1, fights)


def baselines(n_envs=64, n_steps=1000, seed=EVAL_SEED):
    """{"random": (win_rate, fights, mean_length), "cards_first": ...} by `measure`."""
    return {"random": measure(random_legal, n_envs, n_steps, seed), "cards_first": measure(cards_first, n_envs, n_steps, seed)}


def save_checkpoint(net, path, meta):
    """Writes `net.state_dict()` and `meta` (a JSON-able dict: iteration, eval win rate, the
    hyper-parameters) to `path` with torch.save."""
    torch.save({"state_dict": net.state_dict(), "meta": dict(meta)}, path)


def load_checkpoint(path, device=DEVICE):
    """(net, meta) from `save_checkpoint`'s file: a fresh ActorCritic in eval mode on `device`
    with the saved weights."""
    blob = torch.load(path, map_location=device)
    net = ActorCritic().to(device)
    net.load_state_dict(blob["state_dict"])
    net.eval()
    return net, blob["meta"]


def format_row(row):
    """One log line from a dict with COLUMNS' keys (missing ones print as '-'): `it` and
    `steps` as integers, rates to 3 decimals, entropy/clipfrac/value_loss/mean_length to 2,
    approx_kl to 4, sps as an integer, each as `key=value` separated by two spaces."""
    fmt = {"it": "{:d}", "steps": "{:d}", "win_rate": "{:.3f}", "eval_win": "{:.3f}", "entropy": "{:.2f}", "clipfrac": "{:.2f}",
           "approx_kl": "{:.4f}", "value_loss": "{:.2f}", "mean_length": "{:.2f}", "sps": "{:.0f}"}
    parts = []
    for k in COLUMNS:
        v = row.get(k)
        parts.append(f"{k}={'-' if v is None else fmt[k].format(v)}")
    return "  ".join(parts)


def experiment(out_dir, iterations=20, eval_every=5, n_envs=64, T=128, lr=3e-4, seed=0, log=print, **kw):
    """Day 10's loop with the bookkeeping: every iteration a row (COLUMNS) in the returned
    history with `steps` the env steps so far, `mean_length` the mean length of the rollout's
    finished fights (T · n / episodes), `sps` env steps per second of the whole iteration;
    every `eval_every` iterations (and after the last) `eval_win` from day 10's `evaluate`
    on seed EVAL_SEED, and a checkpoint `last.pt`; `best.pt` whenever eval_win improves.
    Writes `history.json` (the list of rows) to `out_dir` at the end. Returns (net, history)."""
    out_dir = pathlib.Path(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    torch.manual_seed(seed)
    envs = make_envs(n_envs, seed=seed)
    net = ActorCritic().to(DEVICE)
    opt = torch.optim.Adam(net.parameters(), lr=lr, eps=1e-5)
    history, steps, best = [], 0, -1.0
    meta = {"n_envs": n_envs, "T": T, "lr": lr, "seed": seed, **kw}
    for it in range(iterations):
        t0 = time.perf_counter()
        batch = rollout(envs, net, T)
        stats = update(net, opt, batch, **kw)
        steps += T * n_envs
        row = {"it": it, "steps": steps, "win_rate": batch["wins"] / max(1, batch["episodes"]), "eval_win": None,
               "entropy": stats["entropy"], "clipfrac": stats["clipfrac"], "approx_kl": stats["approx_kl"],
               "value_loss": stats["value_loss"], "mean_length": T * n_envs / max(1, batch["episodes"]),
               "sps": T * n_envs / (time.perf_counter() - t0)}
        if (it + 1) % eval_every == 0 or it == iterations - 1:
            row["eval_win"], _ = evaluate(net, seed=EVAL_SEED)
            net.train()
            save_checkpoint(net, out_dir / "last.pt", {**meta, "it": it, "eval_win": row["eval_win"]})
            if row["eval_win"] > best:
                best = row["eval_win"]
                save_checkpoint(net, out_dir / "best.pt", {**meta, "it": it, "eval_win": best})
        history.append(row)
        if log:
            log(format_row(row))
    envs.close()
    (out_dir / "history.json").write_text(json.dumps(history, indent=1))
    return net, history


if __name__ == "__main__":
    iterations = int(sys.argv[1]) if len(sys.argv) > 1 else 40
    out = sys.argv[2] if len(sys.argv) > 2 else str(ROOT / "target/learn/day11")
    for name, (wr, n, length) in baselines().items():
        print(f"{name:12s} win {wr:.3f} over {n} fights, {length:.1f} actions each")
    net, history = experiment(out, iterations)
    best, meta = load_checkpoint(pathlib.Path(out) / "best.pt")
    print(f"best checkpoint: iteration {meta['it']}, eval win {meta['eval_win']:.3f}; history in {out}/history.json")
