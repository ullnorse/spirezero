#!/usr/bin/env bash
# Usage: [N=8] [ASC=0] [CHARACTER=Necrobinder] [ACT_INDEX=0] tools/diff_events.sh <EVENT_ID>...
# Forced-event diffs (XT2): for each event, tools/diff_run.sh with EVENT=<id> under the random and the greedy policy, N
# seeds each. The run plays Neow, enters the event from the map (the referee's `enter_event`, the engine's
# `Run::debug_enter_event`), takes first-page option <seed index> % <options> and the policy's picks after it, and stops
# back on the map, so N >= the number of first-page options covers every one. ACT_INDEX=1 enters that act first (a run
# loadout's "act_index", `RunManager.EnterAct`), for events gated to a later act. Prints one summary line per pass and the
# options taken; exits 1 if any seed diverges. Work files in ${DIFF_OUT:-target/diff}/events/<event>-<policy>.
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
[ $# -gt 0 ] || { echo "usage: tools/diff_events.sh <EVENT_ID>..." >&2; exit 2; }
cargo build -q --manifest-path "$root/Cargo.toml" --profile regress -p sts2-diff || exit 2
status=0
loadout=()
if [ "${ACT_INDEX:-0}" != 0 ]; then
    mkdir -p "${DIFF_OUT:-$root/target/diff}/events"
    act_loadout=${DIFF_OUT:-$root/target/diff}/events/act$ACT_INDEX.json
    echo "{\"act_index\": $ACT_INDEX}" > "$act_loadout"
    loadout=(RUN_LOADOUT="$act_loadout")
fi
for event in "$@"; do
    for policy in random greedy; do
        out=${DIFF_OUT:-$root/target/diff}/events/$event-$policy
        log=$out.log
        mkdir -p "$out"
        env "${loadout[@]}" EVENT=$event POLICY=$policy DIFF_OUT=$out DIFF_NO_BUILD=1 DIFF_KEEP=1 "$root/tools/diff_run.sh" "${N:-8}" > "$log" 2>&1 || status=1
        echo "$event $policy: $(grep 'run seeds:' "$log" || tail -1 "$log")"
        grep -E 'DIVERGENCE|pin:' "$log" | head -3
        # The option text keys chosen inside the event, over all seeds.
        python3 -I - "$out/run" <<'PY'
import json, sys
from pathlib import Path
chosen = set()
for f in Path(sys.argv[1]).glob("*.referee.jsonl"):
    lines = [json.loads(l) for l in f.read_text().splitlines() if l.strip()]
    page = None
    for line in lines:
        a = line["action"]
        if page is not None and a.get("action") == "choose_option":
            chosen.add(page[a["args"]["option_index"]])
        pending = (line.get("run") or {}).get("pending") or {}
        page = [o["text_key"] for o in pending.get("options", [])] if pending.get("kind") == "event" and a.get("action") != "start" and line["step"] > 0 and any(l["action"].get("action") == "enter_event" for l in lines[:line["step"] + 1]) else None
print("  options:", ", ".join(sorted(k.split(".options.")[-1] for k in chosen)))
PY
    done
done
exit $status
