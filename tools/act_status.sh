#!/usr/bin/env bash
# What is still missing per act (`cargo xtask coverage --allow-missing`): a table of the missing counts per character x act, and
# a table per kind x act for one character (kinds differ between characters only in their cards).
# Usage: tools/act_status.sh [character for the per-kind table, default Necrobinder]
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root" || exit 2
character=${1:-Necrobinder}
characters="Necrobinder Ironclad Silent Defect Regent"
acts="overgrowth underdocks hive glory"
kinds="card relic potion power monster encounter event enchantment affliction"
cargo build -q -p xtask 2>/dev/null
dir=$(mktemp -d "$root/target/act-status.XXXXXX")
trap 'rm -rf "$dir"' EXIT
for c in $characters; do
    for a in $acts; do
        cargo xtask coverage --character "$c" --act "$a" --allow-missing > "$dir/$c-$a.txt" 2>&1
    done
done
# Missing column (4th) of the `total` or kind row.
missing() { awk -v k="$1" '$1 == k { print $4 }' "$dir/$2-$3.txt"; }

echo "missing per character x act (ported/in scope)"
printf '%-12s' character
for a in $acts; do printf ' %16s' "$a"; done
echo
for c in $characters; do
    printf '%-12s' "$c"
    for a in $acts; do
        read -r _ ported scope miss _ < <(grep '^total' "$dir/$c-$a.txt")
        printf ' %16s' "$miss ($ported/$scope)"
    done
    echo
done
echo
echo "missing per kind ($character)"
printf '%-12s' kind
for a in $acts; do printf ' %10s' "$a"; done
echo
for k in $kinds total; do
    printf '%-12s' "$k"
    for a in $acts; do printf ' %10s' "$(missing "$k" "$character" "$a")"; done
    echo
done
