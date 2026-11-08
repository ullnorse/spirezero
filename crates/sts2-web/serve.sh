#!/bin/sh
# Serves www/ (or $WWW, e.g. a release from deploy.sh) gzipped (serve.py) on the LAN (port $PORT, 8765 by default) with the game text and art linked in at
# www/text, www/art and www/audio (`crates/sts2-web/tools/audio_import.py`) from ~/.cache/spirezero-data (`cargo xtask art-import`; git-ignored, never committed), and
# prints the LAN URL with a QR code (qr.py) for the phone's camera.
set -eu
cachedir() { n="$HOME/.cache/spirezero-$1" o="$HOME/.cache/spirebot-$1"; if [ ! -e "$n" ] && [ -d "$o" ]; then echo "$o"; else echo "$n"; fi; }  # old name while only it exists
www="$(cd "${WWW:-$(dirname "$0")/www}" && pwd)"
text="${STS2_TEXT:-$(cachedir data)/text}"
if [ -d "$text" ] && [ ! -e "$www/text" ]; then ln -s "$text" "$www/text"; fi
art="${STS2_ART:-$(cachedir data)/art}"
if [ -d "$art" ] && [ ! -e "$www/art" ]; then ln -s "$art" "$www/art"; fi
audio="${STS2_AUDIO:-$(cachedir data)/audio}"
if [ -d "$audio" ] && [ ! -e "$www/audio" ]; then ln -s "$audio" "$www/audio"; fi
[ -f "$www/pkg/sts2_web_bg.wasm" ] || echo "no www/pkg yet: run crates/sts2-web/build.sh first" >&2
# The LAN address: Linux's `hostname -I`, else macOS's Wi-Fi (en0) or wired (en1) one.
ip="$(hostname -I 2>/dev/null | cut -d' ' -f1)"
[ -n "$ip" ] || ip="$(ipconfig getifaddr en0 2>/dev/null || ipconfig getifaddr en1 2>/dev/null || echo localhost)"
url="http://$ip:${PORT:-8765}/"
echo "open $url on the phone, or scan:"
python3 -I "$(dirname "$0")/qr.py" "$url"
exec python3 -I "$(dirname "$0")/serve.py" "$www" "${PORT:-8765}" "${BIND:-0.0.0.0}"
