"""CB2: screens a PPO lane's checkpoints on the human panel while it trains.

    python tools/cb2/autobench.py RUN_DIR NAME --every 50 --unit cb2-r

Every minute: once `RUN_DIR/state.pt` holds a checkpoint at or past the next milestone (every
`every` M decisions after the first one seen), it is pinned to `RUN_DIR/pinned/state-<N>M.pt`
and the net alone (greedy, raw weights, 1 rep, seeds 7,000,000 + k) plays the panel's boss and
elite starts (`cb1_mcts.py run --greedy`). One line per milestone goes to
`target/cb2/NAME-screen.tsv`: decisions, panel bosses act 1/2/3/all, elites (deaths %).
Stops when the training unit is no longer active (after a last check).
"""

import argparse
import pathlib
import shutil
import subprocess
import time

import numpy as np
import torch

ROOT = pathlib.Path(__file__).resolve().parents[2]
PY = pathlib.Path.home() / "spirebot/.venv/bin/python"
PANEL = pathlib.Path.home() / ".cache/spirebot-data/cb1/panel_bench.jsonl"


def row(npz):
    z = np.load(npz)
    room, act, died = z["room"], z["act"], z["died"].astype(float)
    boss = room == "boss"
    cells = [100 * died[boss & (act == a)].mean() for a in range(3)]
    cells += [100 * died[boss].mean(), 100 * died[room == "elite"].mean()]
    return "\t".join(f"{c:.1f}" for c in cells)


def bench(ck, name):
    out = ROOT / f"target/cb2/{name}.npz"
    subprocess.run([str(PY), "python/cb1_mcts.py", "run", str(ck), "--greedy", "--reps", "1",
                    "--bench", str(PANEL), "--only", "boss,elite", "--fights", "1024",
                    "--threads", "4", "--weights", "raw", "--out", str(out)],
                   cwd=ROOT, check=True, capture_output=True)
    return row(out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("run")
    ap.add_argument("name")
    ap.add_argument("--every", type=int, default=50)
    ap.add_argument("--unit", required=True)
    a = ap.parse_args()
    run = pathlib.Path(a.run).expanduser()
    tsv = ROOT / f"target/cb2/{a.name}-screen.tsv"
    nxt = None
    while True:
        active = subprocess.run(["systemctl", "--user", "is-active", "-q", a.unit]).returncode == 0
        st = run / "state.pt"
        if st.exists():
            try:
                dec = torch.load(st, map_location="cpu", weights_only=False)["decisions"]
            except (RuntimeError, EOFError, OSError):  # caught mid-write
                time.sleep(30)
                continue
            m = dec // 1_000_000
            if nxt is None:
                nxt = m + a.every
            elif m >= nxt:
                (run / "pinned").mkdir(exist_ok=True)
                pin = run / "pinned" / f"state-{m}M.pt"
                shutil.copy(st, pin)
                line = f"{m}M\t" + bench(pin, f"{a.name}-{m}M-panel1")
                with open(tsv, "a") as f:
                    f.write(line + "\n")
                print(line, flush=True)
                nxt = m + a.every
        if not active:
            break
        time.sleep(60)


if __name__ == "__main__":
    main()
