#!/usr/bin/env bash
# Usage: referee/run.sh <seed> <actions.json> > trace.jsonl
exec python3 -I "$(dirname "$0")/referee.py" run "$@"
