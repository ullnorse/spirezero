#!/bin/bash
# usage: tools/cb1/compare.sh M  -- s1 vs h at M decisions (pinned state-<M>M.pt; waits for both):
# NAZ3 bench bosses and the human-panel bench (bosses + elites), greedy, raw, 8 reps.
set -e
cd "$(dirname "$0")/../.."
M=$1; C=$HOME/spirebot/target/rl-checkpoints/CB1
pin() { ls $C/$1/pinned/state-*M.pt 2>/dev/null | sed -E 's/.*state-([0-9]+)M.pt/\1/' |
        awk -v m=$M '$1>=m-3 && $1<=m+12' | sort -n | head -1; }
until [ -n "$(pin s1)" ] && [ -n "$(pin h)" ]; do sleep 120; done
export NAZ3_GPU_GB=${NAZ3_GPU_GB:-1.2} NAZ3_GPU_CHUNK=${NAZ3_GPU_CHUNK:-256}
PY=$HOME/spirebot/.venv/bin/python
for lane in s1 h; do
  m=$(pin $lane); ck=$C/$lane/pinned/state-${m}M.pt
  echo "== $lane at ${m}M"
  tools/cb1/bench.sh $ck $lane-${m}M-boss --only boss
  tools/cb1/bench.sh $ck $lane-${m}M-panel --bench ~/.cache/spirebot-data/cb1/panel_bench.jsonl >/dev/null || true
done
$PY tools/cb1/panel_report.py target/cb1/s1-$(pin s1)M-panel.npz target/cb1/h-$(pin h)M-panel.npz target/cb1/necro5-panel.npz
$PY python/naz3_combat.py report target/cb1/base-1.5B-r8.npz target/cb1/s1-$(pin s1)M-boss.npz target/cb1/h-$(pin h)M-boss.npz 2>&1 | rg -v "Gym|gymnasium|Please upgrade"
