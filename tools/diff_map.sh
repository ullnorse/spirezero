#!/usr/bin/env bash
# Usage: [ACTS="0 1 2"] [ASCENSION=N] [ACT1=overgrowth|underdocks] tools/diff_map.sh <n_seeds>
# For seeds RL1S0..RL1S<n-1>: start a run on the referee with no actions (step 0 only), then compare
# the maps it dumped with the Rust generator's exactly. Without ACTS, the Act 1 map (`run.map`);
# with ACTS, each listed act index's map from `run.act_maps` (the referee builds every act's
# standard map at step 0, see referee/README.md). Exits non-zero on any divergence. Work files stay
# in ${DIFF_OUT:-target/diff}/map.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
n=${1:?usage: tools/diff_map.sh <n_seeds>}
acts=${ACTS:-}
ascension=${ASCENSION:-0}
act1=${ACT1:-overgrowth}
out=${DIFF_OUT:-$root/target/diff}/map
mkdir -p "$out"
cargo build -q --manifest-path "$root/Cargo.toml" --profile regress -p sts2-diff
bin=$root/target/regress/sts2-diff
act_maps=false
[ -n "$acts" ] && act_maps=true
echo "{\"mode\": \"run\", \"character\": \"Necrobinder\", \"ascension\": $ascension, \"act1\": \"$act1\", \"act_maps\": $act_maps, \"actions\": []}" \
    > "$out/actions.json"

matching=0
diverging=0
for ((i = 0; i < n; i++)); do
    seed=RL1S$i
    "$root/referee/run.sh" "$seed" "$out/actions.json" > "$out/$seed.referee.jsonl"
    for act in ${acts:-1}; do
        args=(--seed "$seed" --referee "$out/$seed.referee.jsonl")
        label=$seed
        if [ -n "$acts" ]; then
            args+=(--act "$act" --ascension "$ascension")
            label="$seed act $act"
        fi
        status=0
        result=$("$bin" compare-map "${args[@]}") || status=$?
        echo "$label: $result"
        case $status in
            0) matching=$((matching + 1)) ;;
            1) diverging=$((diverging + 1)) ;;
            *) echo "$label: compare failed" >&2; exit "$status" ;;
        esac
    done
done
echo "maps: $((matching + diverging)), matching: $matching, diverging: $diverging"
[ "$diverging" -eq 0 ]
