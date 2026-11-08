#!/usr/bin/env bash
# The "done" check for every character x act at once (tools/final_check.sh does one pair, sequentially).
# Usage: tools/final_matrix.sh [--acts act1|hive|glory|both|all] [--hits-only] [n_seeds, default 200]
#   --hits-only (or FINAL_HITS_ONLY=1) runs only the diff_run.sh passes and the zero-hit checks (no gate, regress,
#   regress_runs, coverage, history replay or table): the cheaper way to see an act's zero-hit lists. FINAL_NO_FREEZE=1
#   uses the checkout itself as FINAL_DIR instead of a frozen copy of FINAL_REF (for iterating on uncommitted changes).
#   --acts (or FINAL_MATRIX_ACTS; default act1) the acts signed off: act1 is Overgrowth and Underdocks (natural runs), hive the
#   Hive act-jump cells, glory the Glory act-jump cells, both is act1 and hive, all is act1, hive and glory. The other acts keep
#   their (advisory) coverage line and show "skip" cells.
# On one frozen copy of main (FINAL_DIR, default ~/.cache/spirezero-final-matrix; FINAL_REF, default main): the gate, `cargo xtask
# coverage` for each of the 20 (character, act) pairs, one uncached tools/regress.sh 10 (it covers every character's loadouts)
# and one uncached tools/regress_runs.sh with every CHARACTERS and ACTS value; and the diff_run.sh passes.
# Act 1 (each character x overgrowth/underdocks): the random, greedy and shopper passes at A0 and A10 on RL2S and on the date's
# rotating seeds (ROT<yymmdd>S), plus the real-shop-prices shopper pass, and the community-run replay (tools/replay_history.py,
# skipped without the DB). These are natural runs: they go on into Hive and Glory (XC9), and a
# divergence anywhere in the run fails the cell. The Act 1 zero-hit check (tools/hits_sweep.sh's, docs/hits-allowlist.md)
# runs once over the hit files of these runs (target/final-hits) and of regress; it fails if a ported entity has no hit
# and is not listed.
# Hive (each character): the greedy and shopper passes at A0 and A10 on RL2S and the rotating seeds, started in Hive through
# the act jump with crates/sts2-diff/act_loadouts/hive-<character>.json (see final_pass in tools/final_lib.sh), and the cover
# passes: for every crates/sts2-diff/act_loadouts/cover/hive-<character>-*.json (tools/act_cover.py: the rest of the act's card and
# relic pool in chunks, the act's ancient, forced events) a greedy A0 and a shopper A10 pass (FINAL_COVER_SEEDS seeds, default 6; the
# event and ancient loadouts take all n). Hive's coverage is a hard failure, and so is its own zero-hit check (`--acts hive`, over these
# passes' hit files, target/final-hits-hive, and regress's forced fights, target/regress/work: they hit the monsters, powers and
# encounters an act jump cannot reach), and so is `cover` (no cover pass diverges or lacks a result line).
# Glory (each character): the same (median loadout act_loadouts/glory-<character>.json, an end-of-Act-2 deck, HP, gold and relics:
# the community run nearest the medians, tools/act_loadouts.py; the cover loadouts glory-<character>-*.json); Glory's coverage and its own
# zero-hit check (`--acts glory`, target/final-hits-glory plus regress) are hard failures. A cell also counts the runs that reached the
# Glory boss, the A10 second boss (ascension 10 only) and the victories.
# Below 40 seeds a zero-hit check failure is advisory (rare relics are only hit by random draws).
# The per-pair passes run concurrently, each under tools/slot.sh (so with regress and regress_runs they share
# the machine-wide referee slots; SLOTS, default cores - 2, also sets how many passes are in flight). Every pass has its own log and work dir
# (a passing pass deletes the work dir); divergences are saved as fixture candidates under
# target/fixture-candidates/<pass>/, with the command that pins each in the pass log.
# Logs: FINAL_LOGS (default ~/.cache/spirezero-logs/final-matrix/; <character>-<act>-<policy>-a<asc>-<prefix>.log, gate.log, regress.log,
# regress-runs.log, coverage-*.log, history-replay.log, hits.log, hits-hive.log, hits-glory.log). The summary table (tools/final_table.py) goes to stdout and
# <FINAL_LOGS>.txt; the exit status is 1 on FAIL.
set -uo pipefail
cachedir() { n="$HOME/.cache/spirezero-$1" o="$HOME/.cache/spirebot-$1"; if [ ! -e "$n" ] && [ -d "$o" ]; then echo "$o"; else echo "$n"; fi; }  # old name while only it exists
root=$(cd "$(dirname "$0")/.." && pwd)
. "$root/tools/final_lib.sh"
characters="Necrobinder Ironclad Silent Defect Regent"
acts=$FINAL_ACTS

# One pass: `--job <character> <act> <policy> <asc> <seed prefix> [real]`, run by xargs below.
if [ "${1:-}" = --job ]; then
    shift
    FINAL_LABEL_PREFIX="$1 $2 " final_pass "$FINAL_SEEDS" "$@"
    exit 0
fi

# The rest is one brace group, parsed before it runs: editing this file during a long run cannot break the run.
{
matrix_acts=${FINAL_MATRIX_ACTS:-act1}
if [ "${1:-}" = --acts ]; then
    matrix_acts=${2:?--acts needs act1, hive, glory, both or all}
    shift 2
fi
hits_only=${FINAL_HITS_ONLY:-}
if [ "${1:-}" = --hits-only ]; then
    hits_only=1
    shift
fi
case $matrix_acts in
    act1) run_acts="overgrowth underdocks" ;;
    hive) run_acts=hive ;;
    glory) run_acts=glory ;;
    both) run_acts="overgrowth underdocks hive" ;;
    all) run_acts="overgrowth underdocks hive glory" ;;
    *) echo "--acts: $matrix_acts is not act1, hive, glory, both or all" >&2; exit 2 ;;
esac
act1=false
case " $run_acts " in *" overgrowth "*) act1=true ;; esac
if [ -n "$hits_only" ] && $act1; then
    echo "--hits-only needs --acts hive or glory (the Act 1 check also reads regress's hit files)" >&2
    exit 2
fi
n=${1:-200}
slots=${SLOTS:-$(($(nproc) - 2))}
rotating=$(final_rotating)
export FINAL_DIR=${FINAL_DIR:-$(cachedir final-matrix)} FINAL_SEEDS=$n
[ -z "${FINAL_NO_FREEZE:-}" ] || FINAL_DIR=$root
export FINAL_LOGS=${FINAL_LOGS:-$(cachedir logs)/final-matrix} FINAL_SLOT="$FINAL_DIR/tools/slot.sh"
summary=$FINAL_LOGS.txt
rm -rf "$FINAL_LOGS"
mkdir -p "$FINAL_LOGS"
start=$SECONDS
export DOTNET_ROOT=$HOME/.dotnet PATH=$HOME/.dotnet:$HOME/.dotnet/tools:$PATH

if [ -z "${FINAL_NO_FREEZE:-}" ]; then
    final_freeze "$root" "$FINAL_DIR" "${FINAL_REF:-main}" || exit 2
fi
echo "final matrix ($matrix_acts) at $(git -C "$FINAL_DIR" rev-parse --short HEAD), $n seeds, rotating seeds $rotating, $slots passes at a time"
(cd "$FINAL_DIR" && final_build) || exit 2
# The shared referee build is pinned once for every script below.
STS2_REFEREE=${STS2_REFEREE:-$(python3 -I "$FINAL_DIR/referee/referee.py" where)} || exit 2
export STS2_REFEREE

status=$FINAL_LOGS/status.tsv
: > "$status"
# One line per step: newlines in the text (several result lines) become "; ".
record() { printf '%s\t%s\t%s\n' "$1" "$2" "$(printf '%s' "$3" | sed -z 's/\n*$//; s/\n/; /g')" >> "$status"; }

# regress, regress_runs (their own job pools, also under tools/slot.sh) and the gate run beside the passes.
background= background_runs= gate= history=
# Stale hit files of an unfrozen checkout would count as this run's.
[ -z "${FINAL_NO_FREEZE:-}" ] || rm -rf "$FINAL_DIR/target/regress/work"
regress_seeds=10
[ -z "$hits_only" ] || regress_seeds=${FINAL_HITS_REGRESS_SEEDS:-2}
# --hits-only runs regress too (its forced fights hit the monsters, powers and encounters the act-jump passes cannot reach),
# with FINAL_HITS_REGRESS_SEEDS (default 2) seeds per fight; the rest of the checks are skipped.
(
    cd "$FINAL_DIR" || exit 2
    rc=0
    REGRESS_NO_CACHE=1 tools/regress.sh "$regress_seeds" > "$FINAL_LOGS/regress.log" 2>&1 || rc=$?
    record regress $rc "$(grep -E '^regress: [0-9]+ runs' "$FINAL_LOGS/regress.log" || echo "no result line: $(tail -1 "$FINAL_LOGS/regress.log")")"
    echo "regress: $(grep -E '^regress: [0-9]+ runs' "$FINAL_LOGS/regress.log")"
    grep -E '^FAIL' "$FINAL_LOGS/regress.log" | head -5
) &
background=$!
if [ -z "$hits_only" ]; then
(
    cd "$FINAL_DIR" || exit 2
    rc=0
    REGRESS_NO_CACHE=1 CHARACTERS=$characters ACTS=$acts tools/regress_runs.sh > "$FINAL_LOGS/regress-runs.log" 2>&1 || rc=$?
    record regress_runs $rc "$(grep -E '^regress_runs: [0-9]+ runs' "$FINAL_LOGS/regress-runs.log" || echo "no result line: $(tail -1 "$FINAL_LOGS/regress-runs.log")")"
    echo "regress_runs: $(grep -E '^regress_runs: [0-9]+ runs' "$FINAL_LOGS/regress-runs.log")"
    grep -E '^FAIL' "$FINAL_LOGS/regress-runs.log" | head -5
) &
background_runs=$!
(
    rc=0
    text=$(final_gate "$FINAL_LOGS/gate.log") || rc=$?
    record gate $rc "$text"
    echo "gate: $text"
    for c in $characters; do
        for a in $acts; do
            rc=0
            text=$(final_coverage "$c" "$a" "$FINAL_LOGS/coverage-$c-$a.log") || rc=$?
            record "coverage $c $a" $rc "$text"
        done
    done
) &
gate=$!

# The community-run replay is Act 1 only.
if $act1; then
    (
        rc=0
        text=$(final_history "$FINAL_LOGS/history-replay.log") || rc=$?
        record history $rc "$text"
        echo "$text"
    ) &
    history=$!
fi
fi

# The passes, a pool of $slots: `character act policy asc prefix [real]` per line.
jobs=$(mktemp)
trap 'rm -f "$jobs"' EXIT
for c in $characters; do
    for a in $run_acts; do
        policies="random greedy shopper"
        # Hive and Glory start through the act jump; random play dies too early there to cover the act.
        case $a in hive | glory) policies="greedy shopper" ;; esac
        for asc in 0 10; do
            for p in $policies; do
                echo "$c $a $p $asc RL2S" >> "$jobs"
                echo "$c $a $p $asc $rotating" >> "$jobs"
            done
        done
        case $a in hive | glory) ;; *) echo "$c $a shopper 0 RL2S real" >> "$jobs" ;; esac
        # The cover loadouts (tools/act_cover.py): the rest of the act's card and relic pool, and forced events.
        case $a in
            hive | glory)
                for f in "$FINAL_DIR"/crates/sts2-diff/act_loadouts/cover/$a-"${c,,}"-*.json; do
                    [ -e "$f" ] || continue
                    echo "$c $a greedy 0 cover:$(basename "$f" .json)" >> "$jobs"
                    echo "$c $a shopper 10 cover:$(basename "$f" .json)" >> "$jobs"
                done
                ;;
        esac
    done
done
xargs -a "$jobs" -P "$slots" -L1 "$0" --job
wait
# hits_check <status key> <log> [hive|glory]: the zero-hit check, recorded.
hits_check() {
    local rc=0 text
    text=$(final_hits "$2" ${3:-}) || rc=$?
    # Rare relics are only hit by random draws (the sweep plays 40 seeds per policy), so below 40 seeds a miss is advisory.
    if [ "$rc" -ne 0 ] && [ "$n" -lt 40 ]; then
        text="advisory below 40 seeds: $text"
        rc=0
    fi
    record "$1" $rc "$text"
}
# A divergence or a failed pass of a cover pass fails the matrix as the cells do (they have no cell in the table).
cover_check() {
    local rc=0 bad=() log
    for log in "$FINAL_LOGS"/*-cover-*.log; do
        [ -e "$log" ] || continue
        grep -q 'run seeds:' "$log" && ! grep -qE 'diverging: [1-9]' "$log" || { rc=1; bad+=("$(basename "$log" .log)"); }
    done
    record cover $rc "$(ls "$FINAL_LOGS"/*-cover-*.log 2> /dev/null | wc -l) cover passes${bad:+, failed: ${bad[*]}}"
}
cover_check
! $act1 || hits_check hits "$FINAL_LOGS/hits.log"
case " $run_acts " in *" hive "*) hits_check hits_hive "$FINAL_LOGS/hits-hive.log" hive ;; esac
case " $run_acts " in *" glory "*) hits_check hits_glory "$FINAL_LOGS/hits-glory.log" glory ;; esac
final_save_candidates "$root"

seconds=$((SECONDS - start))
if [ -n "$hits_only" ]; then
    awk -F'\t' '$1 ~ /^(hits|cover|regress)/ { printf "%s: %s  %s\n", $1, ($2 == 0 ? "ok" : "FAIL"), $3; if ($2 != 0) bad = 1 } END { exit bad }' "$status"
    exit $?
fi
FINAL_RUN_ACTS=$run_acts python3 -I "$root/tools/final_table.py" "$FINAL_LOGS" "$seconds" "$n" "$rotating" | tee "$summary"
exit "${PIPESTATUS[0]}"
}
