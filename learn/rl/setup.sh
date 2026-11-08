#!/usr/bin/env bash
# One-time setup for the course: a Python venv at the repo root with numpy, pytest and torch
# (CUDA 12.8 build), and the engine's FFI library. Re-run any time; every step is idempotent.
#   learn/rl/setup.sh            everything
#   learn/rl/setup.sh --no-torch skip torch (needed from day 8)
set -u
cd "$(dirname "$0")/../.."
ROOT=$PWD
TORCH=1
[ "${1:-}" = "--no-torch" ] && TORCH=0

say() { printf '\n== %s\n' "$*"; }
status=()

say "Python"
PY=${PYTHON:-python3}
"$PY" -c 'import sys; assert sys.version_info >= (3, 10), sys.version' || { echo "need python >= 3.10"; exit 1; }
"$PY" --version

say "venv at $ROOT/.venv"
if [ ! -x .venv/bin/python ]; then
  if "$PY" -c 'import ensurepip' 2>/dev/null; then
    "$PY" -m venv .venv
  else
    # Debian/Ubuntu ship python without ensurepip; bootstrap pip by hand.
    "$PY" -m venv --without-pip .venv
    mkdir -p target/setup
    curl -sSL https://bootstrap.pypa.io/get-pip.py -o target/setup/get-pip.py
    .venv/bin/python target/setup/get-pip.py -q
  fi
fi
grep -qx '/.venv' .gitignore || echo '/.venv' >> .gitignore
.venv/bin/python -m pip install -q --upgrade pip
.venv/bin/python -m pip install -q numpy pytest && status+=("numpy+pytest: ok") || status+=("numpy+pytest: FAILED")

if [ "$TORCH" = 1 ]; then
  say "torch (CUDA 12.8 wheels; about 3 GB, a few minutes)"
  if .venv/bin/python -m pip install -q torch --index-url https://download.pytorch.org/whl/cu128; then
    status+=("torch: $(.venv/bin/python -c 'import torch; print(torch.__version__, "cuda" if torch.cuda.is_available() else "CPU ONLY")')")
  else
    status+=("torch: FAILED (no cu128 wheel for this Python? try: .venv/bin/python -m pip install torch)")
  fi
fi

say "FFI library (cargo build --profile ffi -p sts2-ffi)"
if cargo build --profile ffi -p sts2-ffi; then status+=("sts2-ffi: ok"); else status+=("sts2-ffi: FAILED"); fi

say "smoke test"
if .venv/bin/python -m unittest discover -s python/tests 2>&1 | tail -1 | grep -q OK; then
  status+=("sts2 env: ok")
else
  status+=("sts2 env: FAILED")
fi

say "summary"
printf '  %s\n' "${status[@]}"
echo "  run the tests: .venv/bin/python -m pytest learn/rl/exercises/week1"
echo "  open the course: learn/rl/index.html"
