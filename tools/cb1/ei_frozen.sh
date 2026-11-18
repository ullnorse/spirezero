#!/bin/bash
# usage: tools/cb1/ei_frozen.sh BASE R COUNT [search args...]  -- CB1 round 2, step 3: expert
# iteration round R with a frozen value. The search runs the policy of round R-1 (BASE for R = 1)
# with every leaf value from BASE (`--value-ckpt`); the policy alone is distilled toward the
# completed-Q targets (no value loss: --pv 0 --vf 0), on this round's and the previous round's
# data. Then: the net alone (NAZ3 bench, panel; 8 reps) and MCTS on it with BASE's values (panel
# bosses + elites, 1 rep).
set -e
cd "$(dirname "$0")/../.."
base=$1; r=$2; count=$3; shift 3
PY=$HOME/spirebot/.venv/bin/python
export PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True
OUT=$HOME/spirebot/target/rl-checkpoints/CB1/ef; mkdir -p $OUT
DD=$HOME/.cache/spirebot-data/cb1
prev=$base; [ "$r" -gt 1 ] && prev=$OUT/ef$((r - 1)).pt
D=$DD/ef$r
if ! ls $D/ends*.npz >/dev/null 2>&1; then
  $PY python/cb1_mcts.py gen "$prev" --value-ckpt "$base" --starts $DD/train_b.jsonl --count $count \
     --mix 0.5,0.4,0.1 --sims 64 --fights 480 --threads 12 --seed $((100 + r)) --out $D "$@" 2>&1 \
     | rg -v "Gym|gymnasium|migration|Please|^  step" | tail -3 | cut -c1-600
fi
data=$D; [ "$r" -gt 1 ] && data="$DD/ef$((r - 1)) $D"
NAZ3_GPU_GB=7 $PY python/cb1_mcts.py train "$prev" --weights raw --data $data --out $OUT/ef$r.pt --epochs 2 \
   --lr 3e-5 --batch 256 --pv 0 --vf 0 2>&1 | rg "train:|before|epoch|wrote|Error"
tools/cb1/bench.sh $OUT/ef$r.pt ef$r | rg "boss|elite|hallway"
tools/cb1/bench.sh $OUT/ef$r.pt ef$r-panel --bench $DD/panel_bench.jsonl >/dev/null
$PY tools/cb1/panel_report.py target/cb1/ef$r-panel.npz
mkdir -p target/cb1/r2
$PY python/cb1_mcts.py run $OUT/ef$r.pt --value-ckpt "$base" --bench $DD/panel_bench.jsonl --reps 1 \
   --fights 256 --threads 12 --sims 64 --out target/cb1/r2/ef$r-mcts-panel.npz --no-gumbel "$@" 2>&1 | tail -1 | cut -c1-300
$PY tools/cb1/r2_report.py target/cb1/r2/ef$r-mcts-panel.npz
