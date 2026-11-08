# Sourced by tools/regress.sh and tools/regress_runs.sh: the job tally, from what each job printed and xargs's status.
# regress_tally <job list> <results> <xargs status> sets
#   ok    the jobs that printed `ok   <label>`,
#   fails the jobs that printed `FAIL <label>`, plus every listed job that printed neither (killed before its
#         result line: out of memory, a signal), plus one for a failed xargs (it reports a job it could not run or
#         that was killed: 123-127),
#   lost  one `FAIL <label> (no result: killed?)` line per listed job without a result, and the xargs failure.
regress_tally() {
    local list=$1 results=$2 xargs_status=$3 label rest
    local -A seen=()
    ok=0 fails=0 lost=
    while read -r verdict label rest; do
        case $verdict in
            ok) ok=$((ok + 1)); seen[$label]=1 ;;
            FAIL) fails=$((fails + 1)); seen[$label]=1 ;;
        esac
    done <<< "$results"
    while read -r label rest; do
        [ -n "$label" ] && [ -z "${seen[$label]:-}" ] || continue
        fails=$((fails + 1))
        lost+="FAIL $label (no result: the job was killed or crashed)"$'\n'
    done < "$list"
    if [ "$xargs_status" -ne 0 ]; then
        fails=$((fails + 1))
        lost+="FAIL xargs exited $xargs_status (a job could not run or was killed)"$'\n'
    fi
    lost=${lost%$'\n'}
}
