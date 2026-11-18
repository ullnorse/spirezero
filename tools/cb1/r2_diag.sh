#!/bin/bash
# usage: tools/cb1/r2_diag.sh CKPT NAME  -- CB1 round 2, step 2: why MCTS gains little. On the panel
# bosses (1 rep): the net alone and MCTS variants (worlds fresh/paired/one/exact, root m 4/8/16,
# no Gumbel noise at play), with per-fight mistake counts; then the leaf value's calibration per
# act. Summary: tools/cb1/r2_report.py.
cd "$(dirname "$0")/../.."
ck=$1; name=$2
PY=$HOME/spirebot/.venv/bin/python
export PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True
B="--bench $HOME/.cache/spirebot-data/cb1/panel_bench.jsonl --only boss --reps 1 --fights 256 --threads 12"
O=target/cb1/r2; mkdir -p $O
run() { local tag=$1; shift; [ -f $O/$name-$tag.npz ] && return
  $PY python/cb1_mcts.py run $ck $B --out $O/$name-$tag.npz "$@" 2>&1 | tail -1 | cut -c1-300; }
run greedy --greedy
run m4-fresh --sims 64 --m 4
run m4-paired --sims 64 --m 4 --world paired
run m4-exact --sims 64 --m 4 --world exact
run m4-one --sims 64 --m 4 --world one
run m4-paired-nog --sims 64 --m 4 --world paired --no-gumbel
run m8-paired --sims 64 --m 8 --world paired
run m16-paired --sims 64 --m 16 --world paired
$PY tools/cb1/r2_report.py $O/$name-*.npz
[ -f $O/$name-value.json ] || $PY python/cb1_mcts.py value $ck --bench $HOME/.cache/spirebot-data/cb1/panel_bench.jsonl --only boss,elite --reps 1 --fights 256 --threads 12 --out $O/$name-value.json 2>&1 | tail -1 | cut -c1-200
