#!/usr/bin/env bash
# Whole-run parity regression with the run loadouts (crates/sts2-diff/run_loadouts/*.json, format in
# crates/sts2-diff/src/loadout.rs): each one is handed over after Neow (the referee's `run_loadout`),
# then a seeded policy plays the run, on the engine and on the referee, through tools/diff_run.sh.
# Usage: tools/regress_runs.sh [-j JOBS] [-v] [seeds per run, default 8]
#   -j JOBS  runs in parallel (default: half the cores)
#   -v       also list the runs that passed
# Builds and runs target/regress/sts2-diff, as tools/regress.sh. A loadout may set "covers" (entity ids the run reaches by
# draws, e.g. an act's ancients; they only seed the cache closure), "prefer" (a list: diff_run.sh PREFER, random policy only), "policy" (random, greedy or shopper, or a list of them, one run each; default greedy), "seed" (a pinned seed, e.g.
# "ROT261024S193": the run plays that seed alone, with its number as the policy seed, instead of RL2S0..), "ascensions" (default
# 0 and 10), "characters" (default ["Necrobinder"]) and "acts" (default ["overgrowth"]); the
# CHARACTERS and ACTS environment variables (space separated, e.g. ACTS="overgrowth underdocks")
# replace the last two for every loadout. "hive" and "glory" are accepted but skipped with a note until multi-act runs
# work (TODO T2/T3: RUN_ACTS in tools/regress_plan.py). Non-default axes add -<character> and -<act> to the label. Output, cache and environment as tools/regress.sh (the cache keys come from
# tools/regress_plan.py with REGRESS_RUNS=1; a run reaches content by random draws, so the closure
# only covers the loadout and the starters, and REGRESS_NO_CACHE=1 covers the rest; final_check.sh
# uses it). Each matching run leaves <seed>.hits.json under $REGRESS_WORK/runs-<label> (default target/regress/work; tools/hits.sh).
# Failures are counted from exit statuses as in tools/regress.sh: a failed planner, a job with no result and a failed xargs fail the run.
set -uo pipefail
cachedir() { n="$HOME/.cache/spirezero-$1" o="$HOME/.cache/spirebot-$1"; if [ ! -e "$n" ] && [ -d "$o" ]; then echo "$o"; else echo "$n"; fi; }  # old name while only it exists
root=$(cd "$(dirname "$0")/.." && pwd)
export DOTNET_ROOT=$HOME/.dotnet PATH=$HOME/.dotnet:$HOME/.dotnet/tools:$PATH
logs=$root/target/regress
export REGRESS_WORK=${REGRESS_WORK:-$logs/work}
cache=${REGRESS_CACHE:-$(cachedir regress-cache)}

# One diff_run.sh run; `job <label> <ascension> <policy> <loadout> <cache key> <character> <act1>`.
if [ "${1:-}" = --job ]; then
    shift
    label=$1 asc=$2 policy=$3 loadout=$4 key=$5 character=$6 act1=$7
    log=$logs/runs-$label.log
    status=0
    # The loadout's "prefer", and its pinned "seed" as SEED_PREFIX, SEED_FIRST and a count of 1 (else RL2S, 0, REGRESS_SEEDS).
    read -r prefer prefix first count < <(python3 -I -c '
import json, re, sys
data = json.load(open(sys.argv[1]))
seed = re.fullmatch(r"(.*?)([0-9]+)", data.get("seed", "RL2S0"))
print(",".join(data.get("prefer", [])) or "-", seed[1], seed[2] if "seed" in data else 0, 1 if "seed" in data else sys.argv[2])
' "$loadout" "$REGRESS_SEEDS" 2> "$log") || status=$?
    [ "$prefer" != - ] || prefer=
    [ "$status" -ne 0 ] || PREFER=$prefer SEED_PREFIX=$prefix SEED_FIRST=$first ASC=$asc POLICY=$policy CHARACTER=$character ACT1=$act1 RUN_LOADOUT=$loadout DIFF_OUT=$REGRESS_WORK/runs-$label DIFF_NO_BUILD=1 \
        "$root/tools/slot.sh" "$root/tools/diff_run.sh" "$count" > "$log" 2>&1 || status=$?
    if [ "$status" -eq 0 ]; then
        touch "$cache/$key"
        echo "ok   $label"
    else
        echo "FAIL $label (exit $status, see ${log#"$root"/})"
    fi
    exit 0
fi

jobs=$(($(nproc) / 2))
verbose=
while getopts "j:v" opt; do
    case $opt in
        j) jobs=$OPTARG ;;
        v) verbose=1 ;;
        *) echo "usage: tools/regress_runs.sh [-j JOBS] [-v] [seeds]" >&2; exit 2 ;;
    esac
done
shift $((OPTIND - 1))
[ "$jobs" -ge 1 ] || jobs=1
export REGRESS_SEEDS=${1:-8}
mkdir -p "$logs" "$cache"
start=$SECONDS

cargo build -q --manifest-path "$root/Cargo.toml" --profile regress -p sts2-diff || exit 2
STS2_REFEREE=$(python3 -I "$root/referee/referee.py" where) || exit 2
export STS2_REFEREE

list=$(mktemp)
plan=$(mktemp)
# Trace store events of every job's referee (referee/trace_cache.py): `hit`, `miss`, `game`.
export REFEREE_STATS=$logs/.referee.$$
: > "$REFEREE_STATS"
trap 'rm -f "$list" "$plan" "$REFEREE_STATS"' EXIT
REGRESS_RUNS=1 python3 -I "$root/tools/regress_plan.py" "$root" "$REGRESS_SEEDS" "$(basename "$STS2_REFEREE")" \
    "${MAX_ACTIONS:-3000}" ${ASCENSIONS:-0 10} > "$plan" || { echo "regress_runs: tools/regress_plan.py failed" >&2; exit 2; }
[ -s "$plan" ] || { echo "regress_runs: tools/regress_plan.py planned no runs" >&2; exit 2; }
cached=0
while read -r label asc policy loadout key character act1; do
    if [ -z "${REGRESS_NO_CACHE:-}" ] && [ -e "$cache/$key" ]; then
        cached=$((cached + 1))
        [ -z "$verbose" ] || echo "ok   $label (cached)"
    else
        echo "$label $asc $policy $loadout $key $character $act1" >> "$list"
    fi
done < "$plan"

ran=$(wc -l < "$list")
xargs_status=0
results=$(xargs -r -a "$list" -P "$jobs" -L1 "$0" --job) || xargs_status=$?
total=$((ran + cached))
. "$root/tools/regress_lib.sh"
regress_tally "$list" "$results" "$xargs_status"
if [ -n "$verbose" ]; then
    sort <<< "$results" | grep . || true
else
    grep '^FAIL' <<< "$results" | sort || true
fi
[ -z "$lost" ] || echo "$lost"
echo "referee traces: $(grep -c "^hit" "$REFEREE_STATS") from the store, $(grep -c "^miss" "$REFEREE_STATS") played"
echo "regress_runs: $total runs ($cached cached, $ran ran), $((ok + cached)) ok, $fails failed, $((SECONDS - start))s, $REGRESS_SEEDS seeds, $jobs jobs"
[ "$fails" -eq 0 ]
