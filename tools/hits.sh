#!/usr/bin/env bash
# Usage: tools/hits.sh [--min N] [--summary] [--check docs/hits-allowlist.md] [dir...]
# Ported entities that no generated run touched. Aggregates the <seed>.hits.json files that
# tools/diff.sh (and so regress.sh) and tools/diff_run.sh write next to their traces, by default
# under target/regress/work and target/diff, and lists the content enum variants (cards, relics,
# potions, powers, monsters, encounters, events, enchantments, afflictions) that appear in none.
# --min N   also lists the entities hit by fewer than N runs (a run is one seed's hits file) and, per
#           character, the cards of its own pool hit by fewer than N runs played as that character
# --summary the runs and distinct entities hit per kind, per character and per act
# --check F exits non-zero if a ported entity has no hit and is not in the allowlist F (docs/hits-allowlist.md)
# --acts A,B the acts the runs played: a zero-hit entity only other acts' inventories list is advisory
# --effects also report entities whose effect never ran (card played, potion used, power changed):
#           presence alone is not evidence that the behaviour was exercised (hits files from 2026-10-27 on)
# Run regress.sh and diff_run.sh first (a cached regress job keeps the hits of its last real run).
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root" || exit 2
exec cargo xtask hits "$@"
