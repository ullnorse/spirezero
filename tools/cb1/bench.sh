#!/bin/bash
# usage: tools/cb1/bench.sh CKPT NAME [naz3_combat.py run args...]
# NAZ3's benchmark (8 reps, seeds 7,000,000 + k) on CKPT -> target/cb1/NAME.npz, then the report
# against NECRO5 1.5B (target/cb1/base-1.5B-r8.npz).
set -e
cd "$(dirname "$0")/../.."
ck=$1; name=$2; shift 2
PY=$HOME/spirebot/.venv/bin/python
export PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True
$PY python/naz3_combat.py run "$ck" --reps 8 --fights 1024 --threads 4 --weights raw "$@" --out target/cb1/$name.npz 2>&1 | rg -v "Gym|gymnasium|Please upgrade" | tail -2
$PY python/naz3_combat.py report target/cb1/base-1.5B-r8.npz target/cb1/$name.npz 2>&1 | rg -v "Gym|gymnasium|Please upgrade"
