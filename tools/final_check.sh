#!/usr/bin/env bash
# The Act 1 "done" check (docs/STATUS.md): gate, coverage, regress, regress_runs (the run loadouts), and 200-seed random, greedy
# and shopper runs at A0 and A10, all on a frozen copy of main, for one character and Act 1; then a 20-seed greedy run for
# each of the five characters on that act, and the history replay of sampled community runs (skipped without the DB).
# Prints one line per check. tools/final_matrix.sh does this for every character and act at once, in parallel.
# Usage: tools/final_check.sh [n_seeds, default 200] [--character C] [--act A]
#   --character C, --act A  the character and Act 1 every check plays (default Necrobinder, overgrowth; A is
#                           overgrowth, underdocks, hive or glory; Hive and Glory passes start in their act through the act jump, see FINAL_RUN_ACTS in final_lib.sh): coverage, regress (CHARACTER; forced fights have no act),
#                           regress_runs (CHARACTERS, ACTS) and the diff_run passes (CHARACTER, ACT1).
# FINAL_DIR (default ~/.cache/spirezero-final) is the frozen checkout, FINAL_REF (default main) what it is a copy of.
# Regress runs at 10 seeds. Each policy and ascension then runs a second, rotating pass on seeds
# whose prefix comes from the date (ROT<yymmdd>S), so repeated checks do not replay the same seeds;
# a divergence in either pass is saved as a run fixture candidate under target/fixture-candidates
# (copied to this checkout's target below) with the command that pins it. Logs: ~/.cache/spirezero-logs/final-*.log.
# The last step is the hits sweep (tools/hits_sweep.sh: all five characters, both acts, a few seeds each) and its
# zero-hit check. Every step runs to the end; final_check exits 1 if any step failed (the gate, coverage, regress,
# regress_runs, a diff_run pass that diverged or has no result line, the history replay, or the hits sweep: a failed
# job, or a ported entity with no hit that docs/hits-allowlist.md does not excuse), naming the failed steps on its last line.
set -uo pipefail
cachedir() { n="$HOME/.cache/spirezero-$1" o="$HOME/.cache/spirebot-$1"; if [ ! -e "$n" ] && [ -d "$o" ]; then echo "$o"; else echo "$n"; fi; }  # old name while only it exists
root=$(cd "$(dirname "$0")/.." && pwd)
. "$root/tools/final_lib.sh"
n=200
character=Necrobinder
act=overgrowth
while [ $# -gt 0 ]; do
    case $1 in
        --character) character=${2:?--character needs a value}; shift 2 ;;
        --act) act=${2:?--act needs a value}; shift 2 ;;
        *) n=$1; shift ;;
    esac
done
rotating=$(final_rotating)
export FINAL_DIR=${FINAL_DIR:-$(cachedir final)} FINAL_LOGS=$(cachedir logs) FINAL_LOG_PREFIX=final-
mkdir -p "$FINAL_LOGS"
final_freeze "$root" "$FINAL_DIR" "${FINAL_REF:-main}" || { echo "final check: cannot freeze ${FINAL_REF:-main} at $FINAL_DIR" >&2; exit 2; }
cd "$FINAL_DIR" || exit 2
echo "final check at $(git rev-parse --short HEAD), $character $act, rotating seeds $rotating"
export CHARACTER=$character ACT1=$act
# Every step runs; each one that fails (its exit status, not its text) is named here, and any makes the check fail.
failed=()
final_gate "$FINAL_LOGS/final-gate.log" || failed+=(gate)
final_coverage "$character" "$act" "$FINAL_LOGS/final-$character-$act-coverage.log" || failed+=(coverage)
final_build || exit 2
log=$FINAL_LOGS/final-$character-$act-regress.log
REGRESS_NO_CACHE=1 tools/regress.sh 10 > "$log" 2>&1 || failed+=(regress)
grep -E '^regress:' "$log" || echo "regress: no result line: $(tail -1 "$log")"
grep -E '^FAIL' "$log" | head -5
log=$FINAL_LOGS/final-$character-$act-regress-runs.log
REGRESS_NO_CACHE=1 CHARACTERS=$character ACTS=$act tools/regress_runs.sh > "$log" 2>&1 || failed+=(regress_runs)
grep -E '^regress_runs:' "$log" || echo "regress_runs: no result line: $(tail -1 "$log")"
grep -E '^FAIL' "$log" | head -5
for asc in 0 10; do
    for p in random greedy shopper; do
        for prefix in RL2S "$rotating"; do
            final_pass "$n" "$character" "$act" "$p" "$asc" "$prefix" || failed+=("A$asc-$p-$prefix")
        done
    done
done
final_pass "$n" "$character" "$act" shopper 0 RL2S real || failed+=(A0-shopper-realshop)
for c in Ironclad Silent Defect Regent Necrobinder; do
    FINAL_LOG_PREFIX=final-quick- FINAL_LABEL_PREFIX="$c $act " final_pass 20 "$c" "$act" greedy 0 RL2S || failed+=("$c-quick")
done
final_save_candidates "$root"
final_history "$FINAL_LOGS/final-history-replay.log" || failed+=(history)
log=$(cachedir logs)/final-hits.log
status=0
tools/hits_sweep.sh > "$log" 2>&1 || status=$?
grep -E '^(regress|regress_runs|hits_sweep):' "$log"
grep -E '^(card|relic|potion|power|monster|encounter|event|enchantment|affliction): [0-9]+/' "$log" | tr '\n' ';'; echo
if [ "$status" -ne 0 ]; then
    echo "FAIL hits: the sweep failed or a ported entity has zero hits (see $log, docs/hits-allowlist.md)"
    grep -A4 -E '[1-9][0-9]* with 0 hits' "$log" | head -15
    failed+=(hits)
else
    echo "hits: every ported entity is hit or allowlisted"
fi
if [ ${#failed[@]} -gt 0 ]; then
    echo "final check: FAIL (${failed[*]})"
    exit 1
fi
echo "final check: ok"
