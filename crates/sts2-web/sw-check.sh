#!/bin/sh
# Service worker check on localhost (a secure context): offline reload, new build under an open tab,
# cache pruning, clipboard, press feedback. One memory-capped headless Chrome, three builds served in turn.
# Needs build.sh first. Output in $OUT (target/web43 by default). Env: PORT (9004), CDP_PORT (9504), ID (web43).
set -eu
cachedir() { n="$HOME/.cache/spirezero-$1" o="$HOME/.cache/spirebot-$1"; if [ ! -e "$n" ] && [ -d "$o" ]; then echo "$o"; else echo "$n"; fi; }  # old name while only it exists
root="$(cd "$(dirname "$0")/../.." && pwd)"
export OUT="${OUT:-$root/target/web43}" PORT="${PORT:-9004}" CDP_PORT="${CDP_PORT:-9504}"
id="${ID:-web43}"
export SERVE="$root/crates/sts2-web/serve.sh"
avail=$(free -m | awk '/^Mem:/ {print $7}')
[ "$avail" -ge 8192 ] || { echo "only ${avail} MB available: stop (WEB-RULES)" >&2; exit 2; }
cleanup() { systemctl --user stop "$id-chrome.service" "$id-serve.service" 2>/dev/null || true; rm -rf "$OUT/profile"; }
trap cleanup EXIT
mkdir -p "$OUT"; cleanup
rm -rf "$OUT/www" "$OUT/www2" "$OUT/www3"
cp -a "$root/crates/sts2-web/www" "$OUT/www"
# serve.sh links art, text and audio in; do it here so the copies share the links
for d in art text audio; do [ -e "$OUT/www/$d" ] || { s="$(cachedir data)/$d"; [ -d "$s" ] && ln -s "$s" "$OUT/www/$d"; }; done
cp -a "$OUT/www" "$OUT/www2"; echo "// build 2" >>"$OUT/www2/app.js"
cp -a "$OUT/www" "$OUT/www3"; echo "// build 3" >>"$OUT/www3/app.js"
systemd-run --user --unit="$id-chrome" --collect -p MemoryMax=4G -p OOMPolicy=continue google-chrome --headless=new --mute-audio --no-sandbox --disable-gpu \
  --remote-debugging-port="$CDP_PORT" --user-data-dir="$OUT/profile" about:blank >/dev/null
for _ in $(seq 40); do curl -sf "http://127.0.0.1:$CDP_PORT/json/version" >/dev/null && break; sleep 0.5; done
python3 -I "$root/crates/sts2-web/realtest/sw_check.py" | tee "$OUT/sw-results.txt"
