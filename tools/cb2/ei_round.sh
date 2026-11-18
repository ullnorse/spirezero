#!/bin/bash
# usage: tools/cb2/ei_round.sh BASE R COUNT [search args...]  -- CB2 expert-iteration round R.
# The Rust search (64 sims, m 8, paired worlds, Gumbel root, pot_w 0.10) plays COUNT training
# starts (train_b, bosses / elites / hallways 50 / 40 / 10; NAZ3 bench and panel ids excluded)
# with the policy of round R-1 (BASE for R = 1) and BASE's leaf values; the policy alone is
# distilled toward the completed-Q targets (--pv 0 --vf 0). Then the net alone (panel bosses +
# elites, 1 rep) and net + MCTS with BASE's values (panel bosses, then elites, 1 rep each).
set -e
cd "$(dirname "$0")/../.."
base=$1; r=$2; count=$3; shift 3
PY=$HOME/spirebot/.venv/bin/python
export PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True NAZ3_GPU_CHUNK=${NAZ3_GPU_CHUNK:-2048}
OUT=$HOME/spirebot/target/rl-checkpoints/CB2/ei; mkdir -p "$OUT"
DD=$HOME/.cache/spirebot-data/cb2
PANEL=$HOME/.cache/spirebot-data/cb1/panel_bench.jsonl
prev=$base; [ "$r" -gt 1 ] && prev=$OUT/ei$((r - 1)).pt
D=$DD/ei$r
if ! ls "$D"/ends*.npz >/dev/null 2>&1; then
  NAZ3_GPU_GB=${NAZ3_GPU_GB:-7} $PY python/cb2_search.py gen "$prev" --value-ckpt "$base" --count "$count" \
     --seed $((200 + r)) --out "$D" "$@" 2>&1 | rg "^\{|Error" | cut -c1-600
fi
[ -f "$OUT/ei$r.pt" ] || NAZ3_GPU_GB=${NAZ3_GPU_GB:-7} $PY python/cb1_mcts.py train "$prev" --weights raw --data "$D" ${EXTRA_DATA:-} \
   --out "$OUT/ei$r.pt" --epochs 2 --lr 3e-5 --batch 256 --pv 0 --vf 0 2>&1 \
   | rg "train:|before|epoch|wrote|Error"
mkdir -p target/cb2/ei
$PY python/cb1_mcts.py run "$OUT/ei$r.pt" --greedy --reps 1 --bench "$PANEL" --only boss,elite \
   --fights 1024 --threads 4 --weights raw --out target/cb2/ei/ei$r-net.npz >/dev/null 2>&1
for room in boss elite; do
  NAZ3_GPU_GB=${NAZ3_GPU_GB:-7} $PY python/cb2_search.py run "$OUT/ei$r.pt" --value-ckpt "$base" --no-gumbel \
     --bench "$PANEL" --only $room --out target/cb2/ei/ei$r-mcts-$room.npz "$@" 2>&1 \
     | rg "^\{|Error" | cut -c1-200
done
$PY tools/cb2/rows.py target/cb2/ei/ei$r-net.npz target/cb2/ei/ei$r-mcts-boss.npz \
   target/cb2/ei/ei$r-mcts-elite.npz
