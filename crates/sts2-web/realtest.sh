#!/bin/sh
# Real-input test before a deploy: one memory-capped headless Chrome drives the real page (no ?proof=) with CDP
# touch/mouse/wheel/key input at 390x664 (a phone) and 1440x900 (a desktop), the two in parallel in tabs of the same
# Chrome. Each compares the DOM with the engine's view after every action. Exits non-zero on any failure.
# Output: $OUT/results.txt (one line per check) and screenshots, OUT = target/web32 by default.
# Env: PORT (server, 8967), CDP_PORT (9467), HOST (LAN address), ID (systemd unit prefix, web32), BUDGET (s per viewport, 420).
# VIEWS (phone desktop; small is 320x568).
# WWW=<dir> serves that directory (a release) instead of www/.
# Needs crates/sts2-web/build.sh run first. The page is served from the LAN address: plain http, an insecure context.
set -eu
root="$(cd "$(dirname "$0")/../.." && pwd)"
export OUT="${OUT:-$root/target/web32}"
port="${PORT:-8967}"
cdp="${CDP_PORT:-9467}"
id="${ID:-web32}"
host="${HOST:-$(hostname -I | cut -d' ' -f1)}"
export URL="http://$host:$port/" CDP_PORT="$cdp"
mkdir -p "$OUT"
avail=$(free -m | awk '/^Mem:/ {print $7}')
[ "$avail" -ge 8192 ] || { echo "only ${avail} MB available: stop (WEB-RULES)" >&2; exit 2; }
cleanup() {
  systemctl --user stop "$id-chrome.service" "$id-serve.service" 2>/dev/null || true
  rm -rf "$OUT/profile"
}
trap cleanup EXIT
cleanup
systemd-run --user --unit="$id-serve" --collect -p MemoryMax=1G -p OOMPolicy=continue env PORT="$port" ${WWW:+WWW="$WWW"} "$root/crates/sts2-web/serve.sh" >/dev/null
systemd-run --user --unit="$id-chrome" --collect -p MemoryMax=4G -p OOMPolicy=continue google-chrome --headless=new --mute-audio --no-sandbox --disable-gpu \
  --disable-renderer-backgrounding --disable-backgrounding-occluded-windows --disable-background-timer-throttling \
  --remote-debugging-port="$cdp" --user-data-dir="$OUT/profile" about:blank >/dev/null
for _ in $(seq 40); do curl -sf "http://127.0.0.1:$cdp/json/version" >/dev/null && break; sleep 0.5; done
for _ in $(seq 20); do curl -sfI "$URL" >/dev/null && break; sleep 0.5; done
views="${VIEWS:-phone desktop}"
pids=""
for v in $views; do python3 -I "$root/crates/sts2-web/realtest/realtest.py" "$v" & pids="$pids $!"; done
rc=0
for p in $pids; do wait "$p" || rc=1; done
: >"$OUT/results.txt"
for v in $views; do cat "$OUT/results-$v.txt" >>"$OUT/results.txt"; done
echo "$(grep -c '^FAIL' "$OUT/results.txt") failures; results in $OUT/results.txt"
exit $rc
