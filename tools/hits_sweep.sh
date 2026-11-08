#!/usr/bin/env bash
# The hits sweep: every character on both Act 1s at a few seeds, then the zero-hit check.
# Usage: tools/hits_sweep.sh [seeds per forced fight and run loadout, default 2; the whole runs play 20x as many]
# Runs, all at ascension 0 and without the regress cache (a cached run keeps no fresh hits):
#   - tools/regress.sh once per character (every encounter and every combat loadout, as that character),
#   - tools/regress_runs.sh for all five characters and both acts (the run loadouts),
#   - tools/diff_run.sh with the random, greedy and shopper policies for each character and act, and greedy at
#     ascension 10 (Ascender's Bane is in the deck from A5). Events and Neow relics are only reached this way.
# Then `cargo xtask hits --check docs/hits-allowlist.md --acts overgrowth,underdocks --min 3 --summary --effects` over what this
# sweep wrote, and nothing else: target/hits-sweep, emptied first (regress and regress_runs write there through
# REGRESS_WORK). Only matching seeds leave a hits file (tools/diff.sh, tools/diff_run.sh), so content reached after a
# divergence, in a run the referee could not check, or in an older run is never counted. Exits non-zero if a ported
# entity has no hit that the allowlist does not excuse, or if a job failed for any reason but a divergence (a referee
# or harness error: regress or regress_runs exiting 2 or more, a diff_run.sh job exiting 2 or more, or killed); a
# divergence only costs that seed's hits (final_check.sh and regress.sh report divergences). Needs a frozen binary:
# builds the regress profile once.
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root" || exit 2
export DOTNET_ROOT=$HOME/.dotnet PATH=$HOME/.dotnet:$HOME/.dotnet/tools:$PATH
seeds=${1:-2}
characters="Necrobinder Ironclad Silent Defect Regent"
acts="overgrowth underdocks"
out=target/hits-sweep
cargo build -q --profile regress -p sts2-diff || exit 2
rm -rf "$out"
mkdir -p "$out"
export DIFF_NO_BUILD=1 ASCENSIONS=0 REGRESS_NO_CACHE=1 REGRESS_WORK=$root/$out/regress
failed=()
for c in $characters; do
    CHARACTER=$c tools/regress.sh "$seeds" > "$out/regress-$c.log" 2>&1
    status=$?
    grep -E '^regress:' "$out/regress-$c.log" || echo "regress: $c: no result line: $(tail -1 "$out/regress-$c.log")"
    [ "$status" -le 1 ] || failed+=("regress $c (exit $status)")
done
CHARACTERS=$characters ACTS=$acts tools/regress_runs.sh "$seeds" > "$out/regress-runs.log" 2>&1
status=$?
grep -E '^regress_runs:' "$out/regress-runs.log" || echo "regress_runs: no result line: $(tail -1 "$out/regress-runs.log")"
[ "$status" -le 1 ] || failed+=("regress_runs (exit $status)")
# Each job prints `hits_sweep: FAIL ...` and exits 1 when diff_run.sh exits 2 or more (1 is a divergence); xargs then
# exits non-zero, as it does for a job it could not start or that was killed.
status=0
for c in $characters; do
    for a in $acts; do for p in random greedy shopper; do echo "$c $a $p 0"; done; done
    echo "$c overgrowth greedy 10"
done |
    xargs -P 4 -L1 bash -c 'name=$0-$1-$2-a$3; s=0; CHARACTER=$0 ACT1=$1 POLICY=$2 ASC=$3 DIFF_OUT='"$out"'/$name tools/slot.sh tools/diff_run.sh '"$((seeds * 20))"' > '"$out"'/$name.log 2>&1 || s=$?
        [ "$s" -le 1 ] || { echo "hits_sweep: FAIL diff_run $name exited $s (see '"$out"'/$name.log)"; exit 1; }' || status=$?
[ "$status" -eq 0 ] || failed+=("diff_run jobs (xargs exit $status)")
status=0
tools/hits.sh --min 3 --summary --effects --check docs/hits-allowlist.md --acts overgrowth,underdocks "$out" || status=$?
[ "$status" -eq 0 ] || failed+=("hits check")
if [ ${#failed[@]} -gt 0 ]; then
    printf 'hits_sweep: FAIL %s\n' "${failed[@]}"
    exit 1
fi
echo "hits_sweep: ok"
