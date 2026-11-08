#!/bin/sh
# Safe deploy: builds the checked-out commit into a release, tests it, and only then switches the site to it.
#   deploy.sh              build releases/<commit>, play it for a minute with realtest.sh, point `current` at it, restart the unit
#   deploy.sh --full       the whole realtest.sh (about 7 minutes) instead of a minute of play
#   deploy.sh --force      switch even if the build or the tests fail (says so)
#   deploy.sh --rollback   point `current` back at the previous release and restart
# A release is a full www directory: git's www files + built wasm; art, text and audio are links into the data cache.
# The `sts2-web` unit serves $DATA/current (WWW=$DATA/current serve.sh), so merging to main deploys nothing.
# Env (defaults): DATA (~/.cache/spirezero-data: releases/ and current live here), ASSETS (= DATA: art/text/audio source),
#   UNIT (sts2-web), LIVE_PORT (8765), KEEP (3), plus realtest.sh's ID, PORT, CDP_PORT, HOST, OUT for the test run.
set -eu
cachedir() { n="$HOME/.cache/spirezero-$1" o="$HOME/.cache/spirebot-$1"; if [ ! -e "$n" ] && [ -d "$o" ]; then echo "$o"; else echo "$n"; fi; }  # old name while only it exists
root="$(cd "$(dirname "$0")/../.." && pwd)"
DATA="${DATA:-$(cachedir data)}"
ASSETS="${ASSETS:-$DATA}"
UNIT="${UNIT:-sts2-web}"
live="${LIVE_PORT:-8765}"
keep="${KEEP:-3}"
force=0 rollback=0 full=0
for a in "$@"; do
  case "$a" in
    --force) force=1 ;;
    --full) full=1 ;;
    --rollback) rollback=1 ;;
    *) echo "usage: deploy.sh [--full | --force | --rollback]" >&2; exit 2 ;;
  esac
done
mkdir -p "$DATA/releases"

check_live() { # the site answers and serves the page with a build address
  host="${HOST:-127.0.0.1}"
  for _ in $(seq 20); do
    page="$(curl -sf "http://$host:$live/" 2>/dev/null || true)"
    case "$page" in *'<base href="/v/'*) echo "live check: http://$host:$live/ serves $(echo "$page" | rg -o '/v/[0-9a-f]{12}/' | head -1)"; return 0 ;; esac
    sleep 0.5
  done
  echo "live check FAILED: http://$host:$live/ does not serve the page" >&2
  return 1
}

switch() { # switch <release-dir>: atomic link swap, then restart
  new="$1"
  old="$(readlink "$DATA/current" 2>/dev/null || true)"
  if [ -n "$old" ] && [ "$old" != "$new" ]; then ln -sfn "$old" "$DATA/previous.new" && mv -T "$DATA/previous.new" "$DATA/previous"; fi
  ln -sfn "$new" "$DATA/current.new"
  mv -T "$DATA/current.new" "$DATA/current"
  systemctl --user restart "$UNIT.service"
  check_live
}

if [ "$rollback" = 1 ]; then
  prev="$(readlink "$DATA/previous" 2>/dev/null || true)"
  [ -n "$prev" ] && [ -d "$prev" ] || { echo "no previous release to roll back to" >&2; exit 1; }
  cur="$(readlink "$DATA/current" 2>/dev/null || true)"
  # swap: the release we leave becomes `previous`, so a second --rollback goes forward again
  ln -sfn "$cur" "$DATA/previous.new"
  ln -sfn "$prev" "$DATA/current.new"
  mv -T "$DATA/previous.new" "$DATA/previous"
  mv -T "$DATA/current.new" "$DATA/current"
  systemctl --user restart "$UNIT.service"
  echo "rolled back to $(basename "$prev") (was $(basename "$cur"))"
  check_live
  exit 0
fi

cd "$root"
commit="$(git rev-parse --short=12 HEAD)"
branch="$(git branch --show-current)"
fail=""
if [ -n "$(git status --porcelain --untracked-files=no)" ]; then fail="the working tree has uncommitted changes (deploys are of a commit)"; fi
rel="$DATA/releases/$commit"
if [ -z "$fail" ]; then
  rm -rf "$rel"
  mkdir -p "$rel"
  git archive HEAD crates/sts2-web/www | tar -x --strip-components=3 -C "$rel"
  # cargo xtask art-import wipes art/, the home screen icons with it.
  [ -f "$ASSETS/art/app/icon-180.png" ] || python3 crates/sts2-web/tools/icon_import.py || true
  for d in art text audio; do [ -d "$ASSETS/$d" ] && ln -s "$ASSETS/$d" "$rel/$d"; done
  echo "building $commit ($branch) into $rel"
  if ! WWW="$rel" crates/sts2-web/build.sh; then fail="build failed"; fi
fi
if [ -z "$fail" ]; then
  echo "testing $rel"
  if [ "$full" = 1 ]; then quick=""; else quick=1; BUDGET="${BUDGET:-60}"; SCEN="${SCEN:-reward_rows,shop_buy,fight_two,map_pick}"; export BUDGET SCEN; fi
  if ! QUICK="$quick" WWW="$rel" crates/sts2-web/realtest.sh; then
    fail="realtest failed"
    res="${OUT:-$root/target/web32}/results.txt"
    [ -f "$res" ] && rg '^FAIL' "$res" || true
  fi
fi
if [ -n "$fail" ]; then
  if [ "$force" = 1 ]; then
    echo "FORCED: deploying $commit although: $fail" >&2
    [ -d "$rel/pkg" ] || { echo "no build to deploy" >&2; exit 1; }
  else
    echo "NOT deployed: $fail. The live site is unchanged ($(basename "$(readlink "$DATA/current" 2>/dev/null || echo none)"))." >&2
    rm -rf "$rel"
    exit 1
  fi
fi
switch "$rel"
echo "deployed $commit"
# keep the newest $keep releases, never current or previous
cur="$(readlink "$DATA/current")"; prev="$(readlink "$DATA/previous" 2>/dev/null || true)"
ls -1dt "$DATA"/releases/*/ | sed 's:/$::' | tail -n +"$((keep + 1))" | while read -r d; do
  [ "$d" = "$cur" ] || [ "$d" = "$prev" ] || rm -rf "$d"
done
