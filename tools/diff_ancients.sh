#!/usr/bin/env bash
# Usage: [N=40] [ASC=0] [ACT_INDEX=1] [POLICY=random] [SEED_PREFIX=RL2S] [LOADOUT='"remove_relics": [..]'] tools/diff_ancients.sh [CHARACTER]...
# Ancient diffs (XAND): for each character (default Necrobinder), tools/diff_run.sh with a run loadout
# {"act_index": ACT_INDEX, "ancient": true}: the run plays Neow, enters that act (`RunManager.EnterAct`), whose start
# node is its ancient, takes option <seed index> % <options> and the policy's picks after it (card selections), and
# stops back on the map. LOADOUT adds fields to that loadout (`remove_relics`/`remove_cards` for Orobas's locked option). Prints the summary line, then per ancient the options offered and taken over all seeds;
# exits 1 if any seed diverges. Work files in ${DIFF_OUT:-target/diff}/ancients/<character>-a<ASC>.
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cargo build -q --manifest-path "$root/Cargo.toml" --profile regress -p sts2-diff || exit 2
[ $# -gt 0 ] || set -- Necrobinder
base=${DIFF_OUT:-$root/target/diff}/ancients
mkdir -p "$base"
status=0
for character in "$@"; do
    out=$base/$character-a${ASC:-0}${LOADOUT:+-custom}
    loadout=$out.json
    echo "{\"act_index\": ${ACT_INDEX:-1}, \"ancient\": true${LOADOUT:+, $LOADOUT}}" > "$loadout"
    log=$out.log
    mkdir -p "$out"
    RUN_LOADOUT=$loadout CHARACTER=$character POLICY=${POLICY:-random} DIFF_OUT=$out DIFF_NO_BUILD=1 DIFF_KEEP=1 \
        "$root/tools/diff_run.sh" "${N:-40}" > "$log" 2>&1 || status=1
    echo "$character a${ASC:-0}: $(grep 'run seeds:' "$log" || tail -1 "$log")"
    grep -E 'DIVERGENCE' "$log" | head -5
    grep -E '^stop reasons' -A5 "$log" | grep -v ancient_done | tail -n +2
    # Per ancient: the options offered and the ones taken (text keys), over all seeds.
    python3 -I - "$out/run" <<'PY'
import json, sys
from collections import defaultdict
from pathlib import Path
offered, taken = defaultdict(set), defaultdict(set)
for f in sorted(Path(sys.argv[1]).glob("*.referee.jsonl")):
    lines = [json.loads(l) for l in f.read_text().splitlines() if l.strip()]
    for prev, line in zip(lines, lines[1:]):
        pending = (prev.get("run") or {}).get("pending") or {}
        keys = [o["text_key"] for o in pending.get("options", [])]
        if pending.get("kind") != "event" or not keys or keys[0].startswith("NEOW."):
            continue
        ancient = keys[0].split(".")[0]
        offered[ancient] |= {k.split(".options.")[-1] for k in keys}
        if line["action"].get("action") == "choose_option":
            taken[ancient].add(keys[line["action"]["args"]["option_index"]].split(".options.")[-1])
for ancient in sorted(offered):
    print(f"  {ancient}: taken {', '.join(sorted(taken[ancient]))}; offered only {', '.join(sorted(offered[ancient] - taken[ancient])) or '-'}")
PY
done
exit $status
