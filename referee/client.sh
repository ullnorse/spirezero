# Sourced by tools/diff.sh and tools/diff_run.sh: one referee process for all the seeds of the script.
#   referee_trace <seed> <actions.json> <trace.jsonl>   writes the trace; non-zero (reason on stderr) if the referee failed
# The first call starts `referee.py client` as a coprocess (tracer/Reset.cs resets the game between seeds; the
# traces are byte-identical to a fresh process per seed). Under tools/slot.sh the client hands the jobs to the slot's
# long-lived daemon (`referee.py serve --socket`, idle exit after $REFEREE_IDLE s, default 300), so the game starts
# once per slot rather than once per script; otherwise it serves them itself. REFEREE_FRESH=1 runs referee/run.sh per
# seed instead. The coprocess ends when the script exits (its stdin closes).

referee_trace() {
    if [ -n "${REFEREE_FRESH:-}" ]; then
        "$root/referee/run.sh" "$1" "$2" > "$3"
        return
    fi
    if [ -z "${REFEREE_PID:-}" ]; then
        coproc REFEREE { exec python3 -I "$root/referee/referee.py" client; }
    fi
    local status
    printf '%s\t%s\t%s\n' "$1" "$2" "$3" >&"${REFEREE[1]}"
    if ! IFS= read -r status <&"${REFEREE[0]}"; then
        echo "referee process exited" >&2
        return 2
    fi
    [ "$status" = ok ] || { echo "referee: ${status#fail }" >&2; return 1; }
}
