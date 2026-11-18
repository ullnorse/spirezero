#!/bin/sh
# Static export of a web release for Cloudflare Pages (the page behind Access): same addresses as serve.py,
# its request-time work written out as files, `_headers` and `_redirects`.
#   export_static.sh [RELEASE_DIR] OUT_DIR   RELEASE_DIR defaults to ~/.cache/spirezero-data/current (or the spirebot- one while only it exists)
#   export_static.sh --placeholder OUT_DIR   a one-page "private" site, to create the Pages project before the Access login exists
# Files over 25 MiB (art/art.bundle) are left out; it fails if anything still names one. Never touches the release.
set -eu
cachedir() { n="$HOME/.cache/spirezero-$1" o="$HOME/.cache/spirebot-$1"; if [ ! -e "$n" ] && [ -d "$o" ]; then echo "$o"; else echo "$n"; fi; }
here="$(dirname "$0")"
if [ "$#" = 1 ] && [ "$1" != --placeholder ]; then set -- "$(cachedir data)/current" "$1"; fi
exec python3 -I "$here/export_static.py" "$@"
