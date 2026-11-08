#!/usr/bin/env bash
# Run the full verification on a frozen copy of main, so merges in the main checkout don't disturb it.
# Usage: tools/verify_main.sh [seeds] > log
# Exits 1 if any step failed (the gate, regress, regress_runs or a diff_run pass), naming them on its last line.
set -uo pipefail
cachedir() { n="$HOME/.cache/spirezero-$1" o="$HOME/.cache/spirebot-$1"; if [ ! -e "$n" ] && [ -d "$o" ]; then echo "$o"; else echo "$n"; fi; }  # old name while only it exists
logs=$(cachedir logs)
mkdir -p "$logs"
root=$(cd "$(dirname "$0")/.." && pwd)
dir=${VERIFY_DIR:-$(cachedir verify)}
git -C "$root" worktree remove --force "$dir" 2>/dev/null
git -C "$root" worktree add -q --detach "$dir" main || { echo "verify: cannot check out main at $dir" >&2; exit 2; }
cd "$dir" || exit 2
echo "verify $(git rev-parse --short HEAD)"
failed=()
cargo xtask gate 2>&1 | tail -3
[ "${PIPESTATUS[0]}" -eq 0 ] || failed+=(gate)
# A cold regress run takes a long time. `| tail -1` printed nothing until it ended, so a caller that gave
# up first saw no result at all. Log to a file, share the main checkout's (content-addressed) cache so
# the run is mostly cached, and print the summary line, or what went wrong, when it ends.
status=0
tools/regress.sh "${1:-5}" > "$logs/verify-regress.log" 2>&1 || status=$?
[ "$status" -eq 0 ] || failed+=(regress)
grep -E '^(FAIL|regress:)' "$logs/verify-regress.log" | head -6
grep -q '^regress:' "$logs/verify-regress.log" || echo "regress: no result line (exit $status): $(tail -1 "$logs/verify-regress.log")"
tools/regress_runs.sh > "$logs/verify-regress-runs.log" 2>&1 || failed+=(regress_runs)
grep -E '^(FAIL|regress_runs:)' "$logs/verify-regress-runs.log" | head -6
for p in random greedy shopper; do
    POLICY=$p tools/diff_run.sh 100 > "$logs/verify-$p.log" 2>&1 || failed+=("$p")
    echo "$p: $(grep 'run seeds:' "$logs/verify-$p.log" || echo "no result line: $(tail -1 "$logs/verify-$p.log")")"
    grep DIVERGENCE "$logs/verify-$p.log" | head -5
done
if [ ${#failed[@]} -gt 0 ]; then
    echo "verify: FAIL (${failed[*]})"
    exit 1
fi
echo "verify: ok"
