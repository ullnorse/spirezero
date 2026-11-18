#!/bin/bash
# CB2 EI round 1 follow-ups on the same data: the base player's Rust MCTS on panel elites; a gentler
# policy-only distillation (lr 1e-5, 1 epoch; "lo"); a policy + PPO-value distillation on ei1 only
# ("pv"), its value calibrated on the held-out ei1b against the base. Each net: alone and + MCTS.
set -e
cd "$(dirname "$0")/../.."
PY=$HOME/spirebot/.venv/bin/python
export PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True NAZ3_GPU_CHUNK=2048 NAZ3_GPU_GB=7
BASE=$HOME/spirebot/target/rl-checkpoints/CB1/s1/pinned/state-356M.pt
OUT=$HOME/spirebot/target/rl-checkpoints/CB2/ei
DD=$HOME/.cache/spirebot-data/cb2
PANEL=$HOME/.cache/spirebot-data/cb1/panel_bench.jsonl
E=target/cb2/ei
[ -f $E/base356-mcts-elite.npz ] || $PY python/cb2_search.py run $BASE --no-gumbel --bench $PANEL \
   --only elite --out $E/base356-mcts-elite.npz 2>&1 | rg "^\{|Error" | cut -c1-200
bench() {  # name ckpt value-ckpt
  $PY python/cb1_mcts.py run "$2" --greedy --reps 1 --bench $PANEL --only boss,elite --fights 1024 \
     --threads 4 --weights raw --out $E/$1-net.npz >/dev/null 2>&1
  for room in boss elite; do
    $PY python/cb2_search.py run "$2" --value-ckpt "$3" --no-gumbel --bench $PANEL --only $room \
       --out $E/$1-mcts-$room.npz 2>&1 | rg "^\{|Error" | cut -c1-200
  done
  $PY tools/cb2/rows.py $E/$1-net.npz $E/$1-mcts-boss.npz $E/$1-mcts-elite.npz
}
[ -f $OUT/ei1lo.pt ] || $PY python/cb1_mcts.py train $BASE --weights raw --data $DD/ei1 $DD/ei1b \
   --out $OUT/ei1lo.pt --epochs 1 --lr 1e-5 --batch 256 --pv 0 --vf 0 2>&1 | rg "train:|before|epoch|wrote|Error"
bench ei1lo $OUT/ei1lo.pt $BASE
[ -f $OUT/ei1pv.pt ] || $PY python/cb1_mcts.py train $BASE --weights raw --data $DD/ei1 \
   --out $OUT/ei1pv.pt --epochs 2 --lr 3e-5 --batch 256 --pv 1 --vf 0 2>&1 | rg "train:|before|epoch|wrote|Error"
$PY tools/cb2/vcal.py $DD/ei1b $BASE $OUT/ei1pv.pt 2>&1 | rg "mse|Error"
bench ei1pv $OUT/ei1pv.pt ""
