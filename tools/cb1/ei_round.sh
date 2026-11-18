#!/bin/bash
# usage: tools/cb1/ei_round.sh CKPT R [COUNT]  -- one expert-iteration round from CKPT:
# MCTS gen (64 sims, m 4; skipped if the data exists) -> distil (policy + both value heads) ->
# net alone on the NAZ3 bench and the panel (8 reps) -> the MCTS player on the NAZ3 bosses (1 rep).
set -e
cd "$(dirname "$0")/../.."
ck=$1; r=$2; count=${3:-6000}
PY=$HOME/spirebot/.venv/bin/python
D=$HOME/.cache/spirebot-data/cb1/ei$r
OUT=$HOME/spirebot/target/rl-checkpoints/CB1/ei; mkdir -p $OUT
if ! ls $D/ends*.npz >/dev/null 2>&1; then
  $PY python/cb1_mcts.py gen "$ck" --starts ~/.cache/spirebot-data/cb1/train_b.jsonl --count $count \
     --mix 0.5,0.35,0.15 --sims 64 --m 4 --fights 480 --threads 12 --seed $r --out $D 2>&1 2>&1 | rg -v "Gym|gymnasium|migration|Please|^  step" | tail -20 | cut -c1-400
fi
$PY python/cb1_mcts.py train "$ck" --weights raw --data $D --out $OUT/ei$r.pt --epochs 2 --lr 3e-5 --batch 512 2>&1 | rg "train:|before|epoch|wrote"
tools/cb1/bench.sh $OUT/ei$r.pt ei$r | rg "boss|elite|hallway|potions"
tools/cb1/bench.sh $OUT/ei$r.pt ei$r-panel --bench ~/.cache/spirebot-data/cb1/panel_bench.jsonl >/dev/null
$PY tools/cb1/panel_report.py target/cb1/s1-205M-panel.npz target/cb1/ei$r-panel.npz
$PY python/cb1_mcts.py run $OUT/ei$r.pt --only boss --reps 1 --fights 480 --threads 12 --sims 64 --m 4 --out target/cb1/mcts/ei$r-mcts.npz 2>&1 | tail -1 | cut -c1-200
