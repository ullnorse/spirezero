#!/usr/bin/env python3
"""The summary table of tools/final_matrix.sh.

Usage: final_table.py <log dir> <seconds> <seeds> <rotating prefix>

Reads <log dir>/status.tsv (one `key<TAB>exit status<TAB>text` line per global step: gate, regress, regress_runs, and
`history` (the community-run replay), `cover` (the cover passes: no divergence, a result line each), `hits` (the Act 1 zero-hit check), `hits_hive` and `hits_glory` (theirs, over the act-jump passes),
and `coverage <character> <act>` with the missing count as the status' text; lines without the two tabs continue the previous
entry's text; coverage of an act outside FINAL_RUN_ACTS (the acts this matrix signs off) is advisory, marked "(adv)") and the
pass logs written by tools/final_lib.sh (`<character>-<act>-<policy>-a<asc>-<prefix>.log`, `<character>-<act>-shopper-realshop.log`).
An act outside FINAL_RUN_ACTS has "skip" cells; Hive and Glory have no random or real-shop pass ("-"). A Glory cell reads
`matching/total (boss reached, A10 second boss reached, victories)` instead of the Act 2 and Hive boss counts. Prints the table and a final
PASS/FAIL line; the exit status is 1 on FAIL.
"""
import os
import re
import sys
from pathlib import Path

logs, seconds, seeds, rotating = Path(sys.argv[1]), int(sys.argv[2]), sys.argv[3], sys.argv[4]
CHARACTERS = ["Necrobinder", "Ironclad", "Silent", "Defect", "Regent"]
ACTS = ["overgrowth", "underdocks", "hive", "glory"]
JUMP = ["hive", "glory"]  # started through the act jump: greedy and shopper only
ACT1 = ["overgrowth", "underdocks"]
POLICIES = ["random", "greedy", "shopper"]
ASCENSIONS = [0, 10]
# The acts this matrix signs off (tools/final_matrix.sh --acts); the others' coverage does not fail the matrix.
RUN_ACTS = os.environ.get("FINAL_RUN_ACTS", "overgrowth underdocks").split()
GLOBAL = ["gate", "regress", "regress_runs"] + (["history", "hits"] if set(ACT1) & set(RUN_ACTS) else []) + (["cover"] if set(JUMP) & set(RUN_ACTS) else []) + [f"hits_{a}" for a in JUMP if a in RUN_ACTS]
failures = []
LOGS = {"history": "history-replay", "hits_hive": "hits-hive", "hits_glory": "hits-glory"}  # status key -> log name, where they differ


def read_pass(path):
    """(matching, total, act2, hive_boss, hive_beaten, glory_boss, boss2, victory, first divergence line or None); None if
    the pass has no result line.

    act2 counts the seeds that reached Act 2 (`act=` 2 or more on the run-gen stop line), hive_boss those that reached the
    Hive boss (`boss=<a1>,reached|beaten,...`), hive_beaten those that beat it, glory_boss those that reached the Glory boss
    (third field), boss2 the A10 second boss (fourth field) and victory the `stop=victory` ones."""
    if not path.exists():
        return None
    text = path.read_text(errors="replace")
    if text.startswith("skipped:"):
        return "skip"
    m = re.search(r"run seeds: (\d+), matching: (\d+), diverging: (\d+)", text)
    if not m:
        return None
    div = next((line for line in text.splitlines() if "DIVERGENCE" in line), None)
    act2 = len(re.findall(r"; stop=.* act=[2-9] floor=", text))
    hive_boss = len(re.findall(r"; stop=.* boss=[^,]*,(?:reached|beaten)", text))
    hive_beaten = len(re.findall(r"; stop=.* boss=[^,]*,beaten", text))
    glory_boss = len(re.findall(r"; stop=.* boss=[^,]*,[^,]*,(?:reached|beaten)", text))
    boss2 = len(re.findall(r"; stop=.* boss=[^,]*,[^,]*,[^,]*,(?:reached|beaten)", text))
    victory = len(re.findall(r"; stop=victory ", text))
    return int(m[2]), int(m[1]), act2, hive_boss, hive_beaten, glory_boss, boss2, victory, div


def cell(name, glory=False):
    """`matching/total (act2, hive_boss/hive_beaten)` of the pass logs `name` (a list: their sums); records the failures.
    A Glory cell reads `matching/total (glory_boss, boss2, victory)`."""
    sums = [0] * 8
    for n in name:
        r = read_pass(logs / f"{n}.log")
        if r == "skip":
            return "skip"
        if r is None:
            failures.append(f"{n}: no result line")
            return "ERR"
        sums = [a + b for a, b in zip(sums, r[:8])]
        if r[0] != r[1]:
            m = re.match(r"(\S+): DIVERGENCE (?:step \d+ )?(\S+?):", r[8] or "")
            failures.append(f"{n}: {r[1] - r[0]} diverging, first {m[1]} field {m[2]}" if m else f"{n}: {r[1] - r[0]} diverging ({r[8]})")
    if glory:
        return f"{sums[0]}/{sums[1]} ({sums[5]}, {sums[6]}, {sums[7]})"
    return f"{sums[0]}/{sums[1]} ({sums[2]}, {sums[3]}/{sums[4]})"


SKIPPED = "skipped (no multi-act runs yet"  # regress_plan.py's note for a run loadout's act without run passes: not a result


def join(text, more):
    """`text; more`, leaving out the skip notes."""
    kept = [t for t in (text, more.strip()) if t and SKIPPED not in t]
    return "; ".join(kept)


status = {}
last = None
for line in (logs / "status.tsv").read_text().splitlines():
    parts = line.split("\t", 2)
    if len(parts) == 3 and parts[1].lstrip("-").isdigit():
        last = parts[0]
        status[last] = (int(parts[1]), join("", "; ".join(t for t in parts[2].split("; ") if SKIPPED not in t)))
    elif last is not None and line.strip():
        # A continuation of the previous entry's text (an older writer recorded multi-line text).
        rc, text = status[last]
        status[last] = (rc, join(text, line))

header = ["character act", "cov missing"] + [f"{p[:3]} A{a}" for a in ASCENSIONS for p in POLICIES] + ["rotating", "real-shop"]
rows = []
for character in CHARACTERS:
    for act in ACTS:
        pair = f"{character}-{act}"
        rc, text = status.get(f"coverage {character} {act}", (1, "?"))
        parts = text.split()
        missing = parts[3] if len(parts) > 3 and parts[3].isdigit() else "?"
        if rc != 0 and act not in RUN_ACTS:
            missing += " (adv)"  # an act without run passes yet: its coverage is advisory (T2/T3)
        elif rc != 0:
            failures.append(f"coverage {character} {act}: {missing} missing" if missing != "?" else f"coverage {character} {act}: {text}")
        row = [f"{character} {act}", missing]
        if act not in RUN_ACTS:
            rows.append(row + ["skip"] * (len(POLICIES) * len(ASCENSIONS) + 2))
            continue
        # Hive and Glory start through the act jump: greedy and shopper only, no real-shop pass.
        policies = POLICIES if act in ACT1 else ["greedy", "shopper"]
        glory = act == "glory"
        for asc in ASCENSIONS:
            for policy in POLICIES:
                row.append(cell([f"{pair}-{policy}-a{asc}-RL2S"], glory) if policy in policies else "-")
        row.append(cell([f"{pair}-{policy}-a{asc}-{rotating}" for asc in ASCENSIONS for policy in policies], glory))
        row.append(cell([f"{pair}-shopper-realshop"]) if act in ACT1 else "-")
        rows.append(row)

for key in GLOBAL:
    rc, text = status.get(key, (1, "missing"))
    if rc != 0:
        failures.append(f"{key}: failed (see {logs}/{LOGS.get(key, key.replace('_', '-'))}.log)" + (f": {text}" if key in ("history", "hits", "hits_hive", "hits_glory", "cover") else ""))

widths = [max(len(r[i]) for r in [header] + rows) for i in range(len(header))]
out = [f"final matrix: {seeds} seeds per pass, rotating {rotating}, {seconds // 60}m{seconds % 60:02d}s"
       f", acts {' '.join(RUN_ACTS)}"
       " (cells: matching/total seeds (runs that reached Act 2, runs that reached/beat the Hive boss; Glory: runs that reached the Glory boss, the A10 second boss, won); a divergence anywhere in a run fails it)"]
for key in GLOBAL:
    rc, text = status.get(key, (1, "not run"))
    out.append(f"{key}: {'ok' if rc == 0 else 'FAIL'}  {text}")
out.append("")
out.append("  ".join(h.ljust(w) for h, w in zip(header, widths)).rstrip())
out += ["  ".join(c.ljust(w) for c, w in zip(r, widths)).rstrip() for r in rows]
out.append("")
out += [f"FAIL {f}" for f in failures]
out.append(f"final matrix: {'FAIL' if failures else 'PASS'}")
print("\n".join(out))
sys.exit(1 if failures else 0)
