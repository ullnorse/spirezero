#!/bin/bash
# usage: tools/cb1/mcts_bench.sh CKPT NAME SIMS [extra cb1_mcts.py run args]
# MCTS player on the NAZ3 bosses (2 reps) and the human-panel bosses+elites (1 rep), plus the
# one-call latency (1 fight at a time) on 6 bosses.
set -e
cd "$(dirname "$0")/../.."
ck=$1; name=$2; sims=$3; shift 3
PY=$HOME/spirebot/.venv/bin/python
export PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True
run() { $PY python/cb1_mcts.py run "$ck" --sims $sims "$@" 2>&1 | rg -v "Gym|gymnasium|migration|Please upgrade" | tail -1; }
echo "== latency"; run --only boss --limit 6 --reps 1 --fights 1 --threads 8 --out target/cb1/mcts/$name-lat.npz "$@"
echo "== NAZ3 bosses"; run --only boss --reps 2 --fights 256 --threads 12 --out target/cb1/mcts/$name-naz3boss.npz "$@"
echo "== panel"; run --bench ~/.cache/spirebot-data/cb1/panel_bench.jsonl --reps 1 --fights 256 --threads 12 --out target/cb1/mcts/$name-panel.npz "$@"
$PY tools/cb1/panel_report.py target/cb1/mcts/$name-panel.npz
$PY - "$name" <<'PY'
import sys, numpy as np
z = np.load(f"target/cb1/mcts/{sys.argv[1]}-naz3boss.npz")
d = z["died"].astype(float)
print("NAZ3 bosses deaths % act1/2/3/all:", [round(100 * d[z["act"] == a].mean(), 1) for a in range(3)], round(100 * d.mean(), 1))
PY
