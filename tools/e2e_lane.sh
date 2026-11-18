#!/usr/bin/env bash
# The NECRO1 training lane: tools/e2e_lane.sh start|stop|status|pause|resume
# See python/e2e_lane.py for the watchdog and the files in the checkpoint directory.
#
# Env: LANE_OUT (default ~/spirebot/target/rl-checkpoints/NECRO1), LANE_UNIT (the systemd unit,
# default necro-e2e; another lane needs its own, e.g. necro2-e2e for NECRO2), LANE_ARGS (extra
# args for `e2e_lane.py run`: a fresh run's --set config, a smoke test's --seed-range/--pin-at),
# MEMORY_MAX (default below).
set -euo pipefail

UNIT=${LANE_UNIT:-necro-e2e}
ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
PY=${PY:-$HOME/spirebot/.venv/bin/python}
OUT=${LANE_OUT:-$HOME/spirebot/target/rl-checkpoints/NECRO1}
# Measured: the lane (2,048 envs, honesty subprocess included) peaks at 4.14 GB in its cgroup;
# +50% -> 6.2 GB, rounded to 7G.
MEMORY_MAX=${MEMORY_MAX:-7G}

# Python.h for torch.compile's Triton launcher (RLN10); the trainer also adds it.
if [[ -d $HOME/.local/pyinclude ]]; then
  export CPATH="${CPATH:+$CPATH:}$HOME/.local/pyinclude/python3.14:$HOME/.local/pyinclude"
fi

case "${1:-}" in
  start)
    mkdir -p "$OUT"
    if [[ -e "$OUT/STOPPED" ]]; then
      echo "the lane was stopped by its watchdog: $(cat "$OUT/STOPPED")"
      echo "fix the cause, rm $OUT/STOPPED, then start again"; exit 1
    fi
    if systemctl --user is-active --quiet "$UNIT"; then echo "$UNIT is already running"; exit 1; fi
    rm -f "$OUT/PAUSE"
    # The watchdog's start limit counts crash restarts since the last manual start.
    echo "$(date +%s) $(date -Iseconds) MANUAL START" >> "$OUT/lane.log"
    # The trainer loads target/ffi/libsts2_ffi.so; a stale build crash-loops on missing symbols.
    (cd "$ROOT" && cargo build -q --profile ffi -p sts2-ffi) || { echo "ffi build failed"; exit 1; }
    # Freeze the library: merges to main must not change what the lane (and its honesty
    # subprocess) runs, or make the build-id check refuse it. A restart refreezes.
    mkdir -p "$OUT/lib" && cp "$ROOT/target/ffi/libsts2_ffi.so" "$OUT/lib/libsts2_ffi.so"
    BUILD_ID=$("$PY" -c 'import ctypes,sys; l=ctypes.CDLL(sys.argv[1]); l.sts2_build_id.restype=ctypes.c_char_p; print(l.sts2_build_id().decode())' "$OUT/lib/libsts2_ffi.so")
    echo "$(date +%s) $(date -Iseconds) FFI frozen at build $BUILD_ID" >> "$OUT/lane.log"
    # shellcheck disable=SC2086
    systemd-run --user --unit="$UNIT" --collect --working-directory="$ROOT/python" \
      -p Restart=on-failure -p RestartSec=30 -p StartLimitIntervalSec=0 \
      -p RestartForceExitStatus=75 \
      -p MemoryMax="$MEMORY_MAX" -p MemorySwapMax=0 -p Nice=10 -p CPUWeight=20 -p IOWeight=20 \
      -p TimeoutStopSec=180 \
      -E STS2_FFI_LIB="$OUT/lib/libsts2_ffi.so" -E STS2_FFI_BUILD_ID="$BUILD_ID" \
      "$PY" "$ROOT/python/e2e_lane.py" run --out "$OUT" ${LANE_ARGS:-}
    echo "started $UNIT -> $OUT"
    ;;
  stop)
    systemctl --user stop "$UNIT" || true
    echo "stopped (state.pt is saved at SIGTERM)"
    ;;
  status)
    echo "unit        $(systemctl --user is-active "$UNIT" 2>/dev/null || true)"
    # shellcheck disable=SC2086
    "$PY" "$ROOT/python/e2e_lane.py" status --out "$OUT" ${LANE_STATUS_ARGS:-}
    echo "--- lane.log"; tail -n 5 "$OUT/lane.log" 2>/dev/null || true
    ;;
  pause)
    touch "$OUT/PAUSE"; echo "PAUSE set: the trainer saves and frees the GPU after this iteration" ;;
  resume)
    rm -f "$OUT/PAUSE"; echo "PAUSE removed: the trainer resumes within ~5 s" ;;
  *)
    echo "usage: $0 start|stop|status|pause|resume" >&2; exit 2 ;;
esac
