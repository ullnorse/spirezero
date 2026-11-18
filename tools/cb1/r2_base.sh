#!/bin/bash
# usage: tools/cb1/r2_base.sh M...  -- CB1 round 2, step 1: s1 pins state-<M>M.pt alone (NAZ3 bench
# and the panel, 8 reps); then MCTS (64 sims, m 4: tools/cb1/mcts_bench.sh) on the one with the
# fewest panel boss + elite deaths.
cd "$(dirname "$0")/../.."
P=$HOME/spirebot/target/rl-checkpoints/CB1/s1/pinned
PY=$HOME/spirebot/.venv/bin/python
for m in "$@"; do
  ck=$P/state-${m}M.pt
  echo "=== s1 ${m}M net alone"
  tools/cb1/bench.sh $ck s1-${m}M | rg "boss|elite|hallway"
  tools/cb1/bench.sh $ck s1-${m}M-panel --bench ~/.cache/spirebot-data/cb1/panel_bench.jsonl >/dev/null
  $PY tools/cb1/panel_report.py target/cb1/s1-326M-panel.npz target/cb1/s1-${m}M-panel.npz
done
best=$($PY - "$@" <<'PY'
import sys, numpy as np
def score(m):
    z = np.load(f"target/cb1/s1-{m}M-panel.npz")
    return z["died"].mean()
print(min(sys.argv[1:], key=score))
PY
)
echo "=== best net alone on the panel: ${best}M; + MCTS 64 m4"
tools/cb1/mcts_bench.sh $P/state-${best}M.pt s1-${best}M-m4 64 --m 4
