#!/usr/bin/env bash
# Usage: tools/diff.sh <n_seeds> [encounter]
# For seeds D1S0..D1S<n-1>: play a seeded random policy on the Rust engine against `encounter`
# (an engine `Encounter` name such as SlimesWeak, or a referee id such as SLIMES_WEAK; default
# FuzzyWurmCrawlerWeak), replay the same actions on the referee, and compare the traces state by
# state. Exits non-zero on any divergence. The engine side is `target/regress/sts2-diff` (cargo's `regress`
# profile, see Cargo.toml), run once for all the seeds (`gen-batch`, then `compare-batch`). Work files stay in
# ${DIFF_OUT:-target/diff}/[<character>-]<encounter>-a<ASC>[-<loadout>].
# Environment: CHARACTER (a class name such as Ironclad; default Necrobinder), ASC (ascension,
# default 0), LOADOUT (a loadout JSON), MAX_ACTIONS (default 200), DIFF_OUT (work directory root), STS2_REFEREE (referee build, see referee/run.sh).
# Each matching seed also leaves <seed>.hits.json (entity ids touched, see tools/hits.sh): gen-batch writes
# <seed>.hits.pending.json and compare-batch renames it on a match and deletes it on a divergence. A matching seed's two traces
# are deleted (the actions and hits stay; they are most of target/ otherwise); DIFF_KEEP=1 keeps them. A diverging seed is
# saved as a fixture candidate, target/fixture-candidates/<encounter>-a<ASC>[-<loadout>]-<seed>.json,
# and the command that pins it (copying it into crates/sts2-diff/fixtures/) is printed.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
n=${1:?usage: tools/diff.sh <n_seeds> [encounter]}
# CamelCase -> SNAKE_CASE (FuzzyWurmCrawlerWeak -> FUZZY_WURM_CRAWLER_WEAK).
encounter=$(sed -E 's/([a-z0-9])([A-Z])/\1_\2/g' <<< "${2:-FuzzyWurmCrawlerWeak}" | tr '[:lower:]' '[:upper:]')
max_actions=${MAX_ACTIONS:-200}
out=${DIFF_OUT:-$root/target/diff}/${CHARACTER:+$CHARACTER-}$encounter-a${ASC:-0}${LOADOUT:+-$(basename "$LOADOUT" .json)}
mkdir -p "$out"
[ -n "${DIFF_NO_BUILD:-}" ] || cargo build -q --manifest-path "$root/Cargo.toml" --profile regress -p sts2-diff
bin=$root/target/regress/sts2-diff
. "$root/referee/client.sh"

# Three phases, each one process (or one referee session) for all the seeds: play the engine on every
# seed, replay the actions on the referee, compare every trace.
"$bin" gen-batch --prefix D1S --count "$n" --dir "$out" --max-actions "$max_actions" --encounter "$encounter" --ascension "${ASC:-0}" ${CHARACTER:+--character "$CHARACTER"} ${LOADOUT:+--loadout "$LOADOUT"}
for ((i = 0; i < n; i++)); do
    seed=D1S$i
    # A referee that rejects an action still wrote its trace up to the error; `compare` reports that
    # as a divergence. Only a referee that produced nothing is an infrastructure failure.
    referee_trace "$seed" "$out/$seed.actions.json" "$out/$seed.referee.jsonl" || [ -s "$out/$seed.referee.jsonl" ] \
        || { echo "$seed: referee failed" >&2; exit 2; }
done
status=0
results=$("$bin" compare-batch --prefix D1S --count "$n" --dir "$out") || status=$?
[ "$status" -le 1 ] || { echo "compare failed" >&2; exit "$status"; }
echo "$results"
diverging=0
while IFS=: read -r seed result; do
    case $result in
        " ok"*) [ -n "${DIFF_KEEP:-}" ] || rm -f "$out/$seed.rust.jsonl" "$out/$seed.referee.jsonl" ;;
        " DIVERGENCE"*)
            diverging=$((diverging + 1))
            candidate=$root/target/fixture-candidates/$(basename "$out")-$seed.json
            "$bin" fixture --seed "$seed" --actions "$out/$seed.actions.json" --referee "$out/$seed.referee.jsonl" \
                --out "$candidate" > /dev/null &&
                echo "  pin: cp ${candidate#"$root"/} crates/sts2-diff/fixtures/<name>.json" ;;
    esac
done <<< "$results"
echo "$encounter seeds: $n, matching: $((n - diverging)), diverging: $diverging"
[ "$diverging" -eq 0 ]
