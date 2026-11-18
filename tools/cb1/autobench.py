"""CB1: benchmarks a training run's checkpoint at decision milestones while it trains.

    python tools/cb1/autobench.py RUN_DIR NAME --first 50 --every 100 [--unit cb1-s1]

Every minute: when `RUN_DIR/state.pt` has been rewritten at or past the next milestone (in M
decisions: `first`, then every `every`), it is copied to `RUN_DIR/pinned/state-<N>M.pt`, NAZ3's
benchmark runs on it (net alone, greedy, EMA 0.999, 8 reps: `tools/cb1/bench.sh`), and a row of
deaths % is appended to the results table in `docs/research/cb1-combat-bot.md`. Stops when the
training unit is no longer active (after a last check).
"""

import argparse
import pathlib
import shutil
import subprocess
import time

import numpy as np

ROOT = pathlib.Path(__file__).resolve().parents[2]
DOC = ROOT / "docs/research/cb1-combat-bot.md"
MARK = "<!-- autobench rows -->"


def deaths_row(npz, label):
    z = np.load(npz)
    room, act, died = z["room"], z["act"], z["died"].astype(float)

    def pct(sel):
        return f"{100 * died[sel].mean():.1f}"
    boss = room == "boss"
    cells = [pct(boss & (act == a)) for a in range(3)] + [pct(boss), pct(room == "elite"),
                                                          pct(room == "hallway")]
    return f"| {label} | " + " | ".join(cells) + " |"


def append(row):
    text = DOC.read_text()
    DOC.write_text(text.replace(MARK, row + "\n" + MARK, 1))


def main():
    p = argparse.ArgumentParser()
    p.add_argument("run")
    p.add_argument("name")
    p.add_argument("--first", type=int, default=50)
    p.add_argument("--every", type=int, default=100)
    p.add_argument("--unit", default="cb1-s1")
    p.add_argument("--pin-only", action="store_true", help="pin the milestones, run no benchmark")
    a = p.parse_args()
    import torch

    run = pathlib.Path(a.run).expanduser()
    (run / "pinned").mkdir(exist_ok=True)
    nxt, seen = a.first, 0.0
    while True:
        active = subprocess.run(["systemctl", "--user", "is-active", "-q", a.unit]).returncode == 0
        st = run / "state.pt"
        if st.exists() and st.stat().st_mtime != seen:
            seen = st.stat().st_mtime
            time.sleep(20)  # let the save finish
            tmp = run / "pinned" / "probe.pt"
            shutil.copy(st, tmp)
            dec = torch.load(tmp, map_location="cpu", weights_only=False, mmap=True)["decisions"]
            if dec >= nxt * 1e6:
                m = int(round(dec / 1e6))
                pin = run / "pinned" / f"state-{m}M.pt"
                tmp.rename(pin)
                name = f"{a.name}-{m}M"
                if a.pin_only:
                    print(f"pinned {pin}", flush=True)
                    while nxt * 1e6 <= dec:
                        nxt += a.every
                    continue
                out = subprocess.run([str(ROOT / "tools/cb1/bench.sh"), str(pin), name],
                                     capture_output=True, text=True)
                print(out.stdout[-3000:], out.stderr[-2000:], flush=True)
                npz = ROOT / "target/cb1" / f"{name}.npz"
                if npz.exists():
                    append(deaths_row(npz, f"{a.name} {m}M, greedy, raw weights"))
                while nxt * 1e6 <= dec:
                    nxt += a.every
            else:
                tmp.unlink(missing_ok=True)
        if not active:
            return 0
        time.sleep(60)


if __name__ == "__main__":
    raise SystemExit(main())
