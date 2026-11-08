#!/bin/sh
# The phone flow of realtest in Playwright WebKit with an iPhone 13 profile (390x664, touch, scale 3), one
# memory-capped WebKit. WebKit on Linux, not iOS. Needs a venv with playwright (python3 -m venv $VENV; $VENV/bin/pip
# install playwright) and its webkit in ~/.cache/ms-playwright. Output as realtest.sh. Env: ID PORT OUT VENV BUDGET.
set -eu
root="$(cd "$(dirname "$0")/../.." && pwd)"
export OUT="${OUT:-$root/target/web40/wk}"
port="${PORT:-9001}"
id="${ID:-web40}"
venv="${VENV:-$root/target/web40/venv}"
host="${HOST:-$(hostname -I | cut -d' ' -f1)}"
mkdir -p "$OUT"
avail=$(free -m | awk '/^Mem:/ {print $7}')
[ "$avail" -ge 8192 ] || { echo "only ${avail} MB available: stop (WEB-RULES)" >&2; exit 2; }
cleanup() { systemctl --user stop "$id-webkit.service" "$id-serve.service" 2>/dev/null || true; }
trap cleanup EXIT
cleanup
systemd-run --user --unit="$id-serve" --collect -p MemoryMax=1G -p OOMPolicy=continue env PORT="$port" "$root/crates/sts2-web/serve.sh" >/dev/null
for _ in $(seq 20); do curl -sfI "http://$host:$port/" >/dev/null && break; sleep 0.5; done
rc=0
systemd-run --user --unit="$id-webkit" --collect --wait -p MemoryMax=4G -p OOMPolicy=continue -E OUT="$OUT" -E URL="http://$host:$port/" \
  -E BROWSER=webkit -E BUDGET="${BUDGET:-420}" "$venv/bin/python" -I "$root/crates/sts2-web/realtest/realtest.py" phone >"$OUT/log.txt" 2>&1 || rc=1
echo "$(grep -c '^FAIL' "$OUT/results-phone.txt") failures; results in $OUT/results-phone.txt"
exit $rc
