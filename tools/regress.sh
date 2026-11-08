#!/usr/bin/env bash
# Full parity regression against the referee: every ported encounter, then every committed loadout
# on the encounters it names, each at ascension 0 and 10 and at one middle ascension.
# The middle ascension (`mid`, 1 to 9) is picked per run by a stable hash of its name (tools/regress_plan.py, mid_ascension), so
# the ascensions 1-9 (A8/A9 monster scaling, XM1) are each covered by about a ninth of the encounters, and a run keeps its cache
# key and its ascension from one regress to the next (the cache covers it like the others). It adds about half again to the runs
# (4,117 at 5 seeds became 6,172 with the current tree); a loadout limited by "ascensions" that has none of 1-9 has no middle run.
# Usage: tools/regress.sh [-j JOBS] [-v] [seeds per run, default 10]
#   -j JOBS  runs in parallel (default: half the cores)
#   -v       also list the runs that passed
# The engine is built once, with cargo's `regress` profile (Cargo.toml: release speed, debug assertions on),
# and every job runs target/regress/sts2-diff. Quiet by default: the failures (with the log to read) and one summary line. Exits non-zero if
# anything diverges. A run that already passed is skipped; a run that is not still replays the engine, but
# referee/referee.py answers its traces from the trace store (referee/README.md, "Trace store") unless the actions are new. The cache key (shared by every checkout: ~/.cache/spirezero-regress-cache,
# delete it or set REGRESS_NO_CACHE=1 to force a full run; REGRESS_CACHE points elsewhere, e.g. at
# another checkout's cache, which stays valid because the key is content-addressed) is, per run,
# computed by tools/regress_plan.py from
#   - the engine: every source file cargo's dep-info lists for target/regress/sts2-diff (sts2-diff, sts2-core
#     without its tests, sts2-core/build.rs, sts2-macros) without the files directly in content/<kind>/
#     (entity modules, the mod.rs registries, pools.rs) and build.rs's generated output, every crate's
#     Cargo.toml, Cargo.lock, tools/diff.sh, tools/slot.sh, referee/client.sh, referee/referee.py,
#     referee/trace_cache.py, the referee build and MAX_ACTIONS;
#   - the content the run reaches: the entity modules named, transitively, by the loadout, the
#     encounter and the character's starter entities (identifier closure, ~25 of ~500 files);
#   - the loadout file, encounter, ascension and seed count.
# So an engine change re-runs everything, while a merge that only adds or edits entity modules re-runs
# just the loadouts and encounters whose closure contains one (REGRESS_NO_CACHE=1 covers the blind
# spot: a registry edit that changes existing content; final_check.sh uses it). Environment: ASCENSIONS (default
# "0 10 mid"; set it without `mid` for the fixed ascensions alone), CHARACTER (plays every encounter, and every loadout without its own "character", as that
# character; default Necrobinder), MAX_ACTIONS (default 200), STS2_REFEREE (referee build, resolved once for the whole run).
# Forced fights have no act axis: every ported encounter, Act 1, Hive and Glory alike, is played here as it is ported.
# Each matching run also leaves <seed>.hits.json in its work directory (REGRESS_WORK, default target/regress/work; see tools/hits.sh).
#
# Pass or fail comes from exit statuses, never from missing text: a planner that fails, a job that prints no result
# (killed, out of memory) and an xargs that reports a failed or killed job all count as failures. The summary's ok
# count is the jobs that printed `ok`, and it must add up with the failures to the jobs run.
#
# A loadout may name its encounters: {"encounters": ["FuzzyWurmCrawlerWeak", "SlimesWeak"], ...}
# (engine names or referee ids); without the key it runs on those two. `"ascensions": [0]` limits it
# to those ascensions (a loadout of 3 potions does not fit the belt from ascension 4).
# tools/regress_skip.txt lists referee-broken encounters (each with a reason); their runs are skipped and listed.
set -uo pipefail
cachedir() { n="$HOME/.cache/spirezero-$1" o="$HOME/.cache/spirebot-$1"; if [ ! -e "$n" ] && [ -d "$o" ]; then echo "$o"; else echo "$n"; fi; }  # old name while only it exists
root=$(cd "$(dirname "$0")/.." && pwd)
export DOTNET_ROOT=$HOME/.dotnet PATH=$HOME/.dotnet:$HOME/.dotnet/tools:$PATH
logs=$root/target/regress
export REGRESS_WORK=${REGRESS_WORK:-$logs/work}
cache=${REGRESS_CACHE:-$(cachedir regress-cache)}

# One diff.sh run; `job <label> <ascension> <encounter> <loadout|-> <cache key> <character> <act1>`
# (forced fights do not depend on the act).
if [ "${1:-}" = --job ]; then
    shift
    label=$1 asc=$2 encounter=$3 loadout=$4 key=$5 character=$6
    log=$logs/$label.log
    status=0
    ASC=$asc CHARACTER=$([ "$character" = Necrobinder ] || echo "$character") LOADOUT=$([ "$loadout" = - ] || echo "$loadout") DIFF_OUT=$REGRESS_WORK DIFF_NO_BUILD=1 \
        "$root/tools/slot.sh" "$root/tools/diff.sh" "$REGRESS_SEEDS" "$encounter" > "$log" 2>&1 || status=$?
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
        *) echo "usage: tools/regress.sh [-j JOBS] [-v] [seeds]" >&2; exit 2 ;;
    esac
done
shift $((OPTIND - 1))
[ "$jobs" -ge 1 ] || jobs=1
export REGRESS_SEEDS=${1:-10}
mkdir -p "$logs" "$cache"
start=$SECONDS

cargo build -q --manifest-path "$root/Cargo.toml" --profile regress -p sts2-diff || exit 2
# The referee build is pinned for the whole run, so a `setup.sh` elsewhere cannot change it midway.
STS2_REFEREE=$(python3 -I "$root/referee/referee.py" where) || exit 2
export STS2_REFEREE

# Job list: `label asc encounter loadout key` per line, from the planner; the cache decides.
list=$(mktemp)
plan=$(mktemp)
# Trace store events of every job's referee (referee/trace_cache.py): `hit`, `miss`, `game`.
export REFEREE_STATS=$logs/.referee.$$
: > "$REFEREE_STATS"
trap 'rm -f "$list" "$plan" "$REFEREE_STATS"' EXIT
# The planner writes the whole job list first: a planner that dies partway is an error, not a short list.
python3 -I "$root/tools/regress_plan.py" "$root" "$REGRESS_SEEDS" "$(basename "$STS2_REFEREE")" \
    "${MAX_ACTIONS:-200}" ${ASCENSIONS:-0 10 mid} > "$plan" || { echo "regress: tools/regress_plan.py failed" >&2; exit 2; }
[ -s "$plan" ] || { echo "regress: tools/regress_plan.py planned no runs" >&2; exit 2; }
: > "$logs/.cached.$$"
# Referee-broken encounters (tools/regress_skip.txt): `ENCOUNTER_ID[ loadout]` -> reason; a line without a reason is an error.
snake() { sed -E 's/([a-z0-9])([A-Z])/\1_\2/g' <<< "$1" | tr '[:lower:]' '[:upper:]'; }
declare -A skip
while read -r line; do
    entry=${line%%#*} reason=${line#*#}
    read -r enc stem <<< "$entry"
    [ -n "$enc" ] || continue
    [ "$reason" != "$line" ] && [ -n "${reason// }" ] || { echo "tools/regress_skip.txt: no reason for '$line'" >&2; exit 2; }
    skip["$(snake "$enc")${stem:+ $stem}"]=${reason# }
done < "$root/tools/regress_skip.txt"
: > "$logs/.skipped.$$"
while read -r label asc encounter loadout key character act1; do
    enc=$(snake "$encounter") stem=$([ "$loadout" = - ] || basename "$loadout" .json)
    reason=${skip["$enc"]:-${stem:+${skip["$enc $stem"]:-}}}
    if [ -n "$reason" ]; then
        echo "skip $label ($reason)" >> "$logs/.skipped.$$"
        continue
    fi
    if [ -z "${REGRESS_NO_CACHE:-}" ] && [ -e "$cache/$key" ]; then
        echo "cached" >> "$logs/.cached.$$"
        [ -z "$verbose" ] || echo "ok   $label (cached)"
    else
        echo "$label $asc $encounter $loadout $key $character $act1" >> "$list"
    fi
done < "$plan"
cached=$(wc -l < "$logs/.cached.$$")
skipped=$(cat "$logs/.skipped.$$")
rm -f "$logs/.cached.$$" "$logs/.skipped.$$"

xargs_status=0
results=$(xargs -r -a "$list" -P "$jobs" -L1 "$0" --job) || xargs_status=$?
ran=$(wc -l < "$list")
total=$((ran + cached))
. "$root/tools/regress_lib.sh"
regress_tally "$list" "$results" "$xargs_status"
if [ -n "$verbose" ]; then
    sort <<< "$results" | grep . || true
else
    grep '^FAIL' <<< "$results" | sort || true
fi
[ -z "$lost" ] || echo "$lost"
[ -z "$skipped" ] || echo "$skipped"
echo "referee traces: $(grep -c "^hit" "$REFEREE_STATS") from the store, $(grep -c "^miss" "$REFEREE_STATS") played"
echo "regress: $total runs ($cached cached, $ran ran), $((ok + cached)) ok, $fails failed, $(grep -c . <<< "$skipped") skipped (tools/regress_skip.txt), $((SECONDS - start))s, $REGRESS_SEEDS seeds, $jobs jobs"
[ "$fails" -eq 0 ]
