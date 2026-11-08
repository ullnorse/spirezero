#!/usr/bin/env bash
# Run a command while holding one of a machine-wide pool of job slots, so that every checkout's
# regress runs together use at most SLOTS referee processes (default: the core count minus 2).
# Usage: tools/slot.sh <command> [args...]. Slots are flock(1) locks in ~/.cache/spirezero-slots;
# a lock is released when the command exits, even if it is killed. The command gets SPIREZERO_SLOT_SOCKET (and the old SPIREBOT_SLOT_SOCKET), the slot's
# referee daemon (referee/client.sh): one game process per slot, kept between commands.
set -euo pipefail
cachedir() { n="$HOME/.cache/spirezero-$1" o="$HOME/.cache/spirebot-$1"; if [ ! -e "$n" ] && [ -d "$o" ]; then echo "$o"; else echo "$n"; fi; }  # old name while only it exists
slots=${SLOTS:-$(($(nproc) - 2))}
dir=$(cachedir slots)
mkdir -p "$dir"
while :; do
    for i in $(seq 1 "$slots"); do
        exec {fd}> "$dir/$i"
        if flock -n "$fd"; then
            SPIREZERO_SLOT_SOCKET=$dir/$i.sock SPIREBOT_SLOT_SOCKET=$dir/$i.sock "$@"
            exit
        fi
        exec {fd}>&-
    done
    sleep 0.5
done
