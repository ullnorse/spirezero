#!/bin/sh
# Headless proof: serves www/ on localhost, opens each scenario of www/proof.js in headless Chrome
# at a phone's viewport (SIZE, 390x844 by default, 3x) and writes $OUT/<scenario>.png (OUT is
# target/web1 by default), plus the page's final title (`proof:<scenario>:<outcome>:...`) to
# $OUT/titles.txt.
# Usage: crates/sts2-web/proof.sh [scenario...]   (SEED, CHAR, THEME, TEXT, SIZE pick the run, theme, text size, viewport)
# Without SEED/CHAR a scenario plays the run that reaches its screen (`seed_for` below).
set -eu
root="$(cd "$(dirname "$0")/../.." && pwd)"
out="${OUT:-$root/target/web1}"
size="${SIZE:-390x844}"
mkdir -p "$out"
port="${PORT:-8799}"
log="$out/.serve.log"
: >"$log"
PORT="$port" BIND=127.0.0.1 "$root/crates/sts2-web/serve.sh" >/dev/null 2>"$log" &
server=$!
trap 'kill $server 2>/dev/null' EXIT
sleep 1
# The seed and hero that reach a scenario's screen: PROOF dies on floor 2-7, so the deep ones
# (a fight's card choice, treasure, an elite, the boss) play a run that gets there.
seed_for() {
  case "$1" in
    choice|treasure|swipefold) echo "W3B 4" ;;
    elite|boss|act2map|act2fight|botrun|botodds|botwatchplay) echo "W3B 2" ;;
    *) echo "PROOF 4" ;;
  esac
}
cdp() { python3 -I "$root/crates/sts2-web/cdp.py" "$@"; }
scenarios="${*:-start tutorial neow fight target hold enemy turn won choice potion map rewards cardpick event rest treasure shop shopcards menu deck mapsheet help end dead elite boss perf}"
for s in $scenarios; do
  set -- $(seed_for "$s")
  case "$s" in
    bot*)
      # The bot's scenarios wait on its Web Worker, which virtual time does not run: real time.
      name="$s${SIZE:+-$SIZE}"
      title=$(cdp "http://127.0.0.1:$port/?proof=$s&seed=${SEED:-$1}&char=${CHAR:-$2}${THEME:+&theme=$THEME}" "$out/$name.png" "$size" 600)
      echo "$name $title" | tee -a "$out/titles.txt"
      continue
      ;;
  esac
  url="http://127.0.0.1:$port/?proof=$s&seed=${SEED:-$1}&char=${CHAR:-$2}${THEME:+&theme=$THEME}${TEXT:+&text=$TEXT}${PRACTICE:+&practice}"
  name="$s${THEME:+-$THEME}${TEXT:+-$TEXT}${SIZE:+-$SIZE}"
  if [ "$s" = perf ]; then
    # Timing needs real time (virtual time stops the clock); the page also reports its title to
    # the server's log, which is the outcome used if the title did not arrive in time.
    title=$(cdp "$url&report" "$out/$name.png" "$size" 30)
    case "$title" in proof:perf:*) ;; *) title=$(grep "^proof:perf:" "$log" | tail -1) ;; esac
  else
    title=$(cdp "$url" "$out/$name.png" "$size" 60 20000)
  fi
  echo "$name $title" | tee -a "$out/titles.txt"
done
