#!/usr/bin/env bash
# One-screen status of the RL work in progress: each task/RLB* worktree's commits, its training
# curve (last lines), its evaluation summaries, and the RL processes running now.
# Usage: tools/rl_status.sh            (watch -n 60 tools/rl_status.sh to follow it)
set -u
root=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
ckpt="$root/target/rl-checkpoints"

git -C "$root" worktree list --porcelain | awk '/^worktree /{w=$2} /^branch refs\/heads\/task\/RLB/{print w}' |
while read -r wt; do
    id=$(basename "$wt")
    echo "== $id  ($(git -C "$wt" log -1 --format='%h %cr'))"
    git -C "$wt" log --oneline main..HEAD | head -5 | sed 's/^/   commit  /'
    for curve in $(ls -t "$ckpt/$id"/*/curve.tsv 2>/dev/null | head -1); do
        echo "   curve   $curve (update steps secs steps/s ... win ...)"
        tail -3 "$curve" | cut -f1-8 | sed 's/^/           /'
    done
    for s in $(ls -t "$ckpt/$id"/*/summary.txt "$wt"/target/*/summary.txt "$wt"/target/*/*/summary.txt 2>/dev/null | head -6); do
        printf '   eval    %-24s %s\n' "$(basename "$(dirname "$s")")" "$(grep -m1 '^runs' "$s")"
    done
    for p in $(ls -t "$ckpt/$id"/*/progress.tsv "$wt"/target/*/progress.tsv "$wt"/target/*/*/progress.tsv 2>/dev/null | head -3); do
        d=$(dirname "$p")
        [ -f "$d/summary.txt" ] && continue
        printf '   running %-24s %s runs, %s wins\n' "$(basename "$d")" "$(wc -l <"$p")" "$(grep -c victory "$p")"
    done
done

echo "== processes"
ps -eo pid,etime,pcpu,rss,args --sort=-pcpu | grep -E 'sts2-diff|train_|spirebot/.venv|rl-checkpoints| bot --first' | grep -v grep |
    awk '{printf "   %s %s %s%% %dMB", $1, $2, $3, $4/1024; for (i=5; i<=9 && i<=NF; i++) printf " %s", $i; print ""}' | head -8
nvidia-smi --query-gpu=utilization.gpu,memory.used --format=csv,noheader 2>/dev/null | sed 's/^/== gpu  /'
