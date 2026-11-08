#!/usr/bin/env bash
# Usage: referee/find_seed.sh [ENCOUNTER_ID] [--start N] [--limit N]
# Prints the first seed (T3S<n>) whose natural first Act 1 fight is ENCOUNTER_ID.
enc=${1:-FUZZY_WURM_CRAWLER_WEAK}; shift || true
exec python3 -I "$(dirname "$0")/referee.py" find-seed "$enc" "$@"
