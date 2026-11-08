# sts2-web

The phone browser client (docs/specs/ui-reimagined.md section 3.4). The wasm side is the run and
`sts2-view`'s client state: `Web.view()` returns the screen (`Ui::screen` in a fight,
`Ui::room_screen` in a room, `rooms::end_screen` at the run's end), the sheet over it
(`Ui::sheet_screen`) and the save, as JSON; `input(json)` takes an `Input`, and `open(name)` opens a
sheet. `www/` draws it, mobile-first: one portrait column, thumb-reachable actions, no hover.

## Open it on a phone
    rustup target add wasm32-unknown-unknown
    cargo install wasm-bindgen-cli --version 0.2.129 --locked   # must match the wasm-bindgen crate
    crates/sts2-web/build.sh                                    # writes www/pkg (git-ignored)
    crates/sts2-web/serve.sh                                    # port 8765 on the LAN, gzipped, prints the URL and a QR code

On the phone, on the same Wi-Fi, scan the QR code (`qr.py`, stdlib) or open the printed
`http://<LAN IP>:8765/`. If it does not load,
allow the port in the firewall (`sudo ufw allow 8765/tcp`). Then **Add to Home Screen** (Safari:
Share → Add to Home Screen; Chrome: ⋮ → Install app): it opens full screen like an app.

Game text: `serve.sh` links `~/.cache/spirezero-data/text` (from `cargo xtask art-import`) in as
`www/text` (git-ignored; never commit game text). Without it, names come from the engine. Settings
on the start screen show how many tables loaded.

Offline: the service worker caches the page, the wasm and the text on the first visit. Chrome
registers a service worker only on `https` or `localhost`, so on a LAN IP the page plays but is
not installable/offline in Chrome; Safari's Add to Home Screen works on plain http. For the full
PWA over the LAN, forward the port to the phone's localhost (`adb reverse tcp:8765 tcp:8765`, then
`http://localhost:8765/`) or put it behind an https tunnel.

## Deploying (`deploy.sh`)
The site on :8765 is the `sts2-web` user unit, which serves `~/.cache/spirezero-data/current` (a link to a release), not the
checkout: **merging to main deploys nothing.** To publish the checked-out, committed `main`:

    crates/sts2-web/deploy.sh             # build releases/<commit>, run realtest.sh on it, then switch `current` and restart
    crates/sts2-web/deploy.sh --force     # switch even when the build or tests fail (prints that it did)
    crates/sts2-web/deploy.sh --rollback  # back to the previous release (a second --rollback goes forward again)

A release is `~/.cache/spirezero-data/releases/<commit>`: the commit's `www/` files and the built wasm, with `art`, `text`
and `audio` linked in from the data cache. On a failed build or test the script lists the `FAIL` lines, removes the
release, exits non-zero and leaves the live site as it was. The newest 3 releases are kept. Saves and `/v/<build>/` addresses are
unchanged (the build hash is of the content, so the same files give the same address); a tab open across a deploy
keeps working as before. The unit is started with:

    systemd-run --user --unit=sts2-web --collect -p WorkingDirectory=$HOME/spirebot \
      env WWW=$HOME/.cache/spirezero-data/current PORT=8765 crates/sts2-web/serve.sh

(`serve.sh`/`serve.py` are still read from the checkout at each restart.) `DATA`, `ASSETS`, `UNIT`, `LIVE_PORT` and
realtest's `ID`, `PORT`, `CDP_PORT` let a test deploy run against another port and unit.

## What it draws
- **Fight**: one screen, no scrolling with up to 10 cards: enemies as cards (intent, HP bar, block,
  powers), the incoming line (`−7 HP → 59`, lethal in red), you and Osty, the last play, and the
  hand down at the thumb as card tiles framed by type (an attack's red top, a power's round gold
  halo, a curse's dashes; upgraded names green) and by rarity (`CardFace::rarity`: a starter
  plain, common silver ◇, uncommon blue ◆, rare gold ◆ with a sheen). A lone elite or boss is a
  big card with its tag; the Defect's orbs are tokens, one per slot. Tap a card; one that needs a target lights the
  enemies, each showing its HP after the hit (the hint bar sits under them); tap one. Or **drag** a card up: it
  rides above the thumb, the enemy under it lights gold (one target: anywhere above the hand), a card with no
  target lights the whole field; drag it back down to keep it (`Input::Arm`, `Input::Drop`). **Hold** a card, or tap an enemy, yourself
  or Osty, for a sheet with the full text, the next move, the powers and the game's own glossary
  for every lit keyword (`Web.glossary`). After End Turn the next turn starts at once (no banner);
  damage and block float over the bars and the phone buzzes when you are hit. End Turn,
  energy and the labelled draw/discard piles sit in a fixed bottom dock. A card choice is a pinned bottom sheet ("Look"
  folds it to see the field). Each intent is a chip of its kind, told by the glyph `intent_glyph`
  starts its span with: an attack red with a sword and its number big (`6×3`, the hits small),
  block blue, buff and heal green, debuff purple, a status card purple and dashed, summon and
  escape gold, stun yellow, sleep and unknown grey, each with the game's intent icon when the art
  is there (`intent/BUFF`…; it has none for an attack) and a drawn one when not.
  On a phone (< 600 px) the fight fits one screen at any size, everything scaled from the 390 × 664 layout: potions and relics share one row, the
  field takes the height the rest leaves (a size container) and the enemies sit at its bottom,
  over you and the hand, so aiming stays under the thumb; each is a column whose picture grows up
  into the field (whole for one or two, large for three, half the height each for four or five in
  two rows) and gives way first, so the field never overflows. You are one slim row,
  the incoming HP is only End Turn's hint, and the hand is two fixed rows of short tiles sized to
  their text (cost, full name, two lines; five to a row the cost sits in the name's first line),
  three to a row up to six cards, five to a row from seven; a card you can't play now is grey.
- **Start**: Continue, five hero chips with the chosen one's panel, ascension and seed; Start
  stays at the thumb.
- **Rooms**: the choices sit at the bottom, the story above, larger on a phone; the space between shows
  what the choice is about (WEB30): a card reward's cards as tiles (`Web.peek` draws what opening the reward
  shows; a tile's tap takes that card), a rest site's HP now and after resting, a treasure's relic or an
  ancient's relics large with their pictures. Neow and ancients, events, rewards (icons for gold, potions,
  relics and cards, a potion's or relic's text inline; `›` on what opens a choice; the card reward reads
  "Add a card to your deck") and card picks (cards as tiles), rest, treasure, deck selections;
  Proceed/Leave is the dock's button. A button that asks first turns gold and says "Tap again".
- **Shop**: its whole stock on one scrolling screen (`Web.set_flat_shop(true)`, `sts2-view`'s
  `Ui::set_flat_shop`; the TUI keeps the two levels): the cards as tiles with their prices,
  then the colorless cards, the relics and potions (picture, name, text, price), then card removal. A
  sale's price and card are lit green, what you cannot afford is dimmed. A tap asks ("Tap again"), the
  second buys and the screen redraws in place; hold (or the `i`) reads an item, a card's hold shows its
  upgrade.
- **Map**: a vertical, scrollable map in the act's own page, as wide as the screen (on a phone the page runs
  past the edges, the nodes stay on the paper; ≤ 760 px on a laptop), the boss's slot at the top, the path walked
  in ink, you tagged with the hero's marker, the paths to the reachable rooms teal; `Act N` by the title; it glides
  from you to the next row on arrival; on a phone (< 600 px) you tap a next room on the map itself (one tap travels; the dock is empty, so the map gets its height); a laptop keeps the next rooms as dock buttons for the keyboard. Over the map a chip per room
  kind with the **range over every path to the boss** (`MapKind::range`, e.g. elites `0–3`; `sts2-view`'s DAG
  pass, per kind min and max). **Route planner** (`www/map.js`, `map.css`): hover a room (mouse) or tap it (touch)
  to light every path through it, the rest dimmed, the chips switching to that room's ranges
  (`MapCell::through`) and the bar naming it. Mouse: click a next room to travel. Touch: the first tap only
  previews, the second tap on a next room, the **Travel** button in the bar or its dock button travels (a stray
  thumb on a scrolling map must not commit a room). Tap the kind chips to light a kind.
- **Belt**: the potions by name (lit green when one can be drunk now), empty slots dashed, and one
  row of every relic's icon under it (wrapped, smaller as they pile up, none hidden); tap one for its text and glossary. A reward's or shop's relic or potion has an `i` (or hold the row).
- **Sheets**: the run menu (deck, relics, potions and powers, map, log, piles, rooms visited, help,
  settings, save & quit), potion details (use, throw away) as bottom sheets. The deck and the
  piles are card tiles (`DetailSheet::cards`), identical cards stacked ×N, sorted by type, cost
  or name.
  **Swipe** (WEB31, `www/platform.js`): drag a sheet's head or grabber down (or its body, when scrolled to the top): it follows the finger, closes past a third of its height or on a fling, else springs back. On a pinned choice sheet the same drag folds it ("Look"); dragging up unfolds it. Reduced motion: no follow, the gesture still works.
- **Play feel**: a played card's ghost flies to its target (or to you, or into the field) and
  fades; a hit enemy flashes and shakes; block gained glows; the energy orb pulses while a card
  can be played and squeezes when spent; a new hand deals in.
- **Moments**: screens slide (a room), zoom (a fight) or rise (the map) in; an elite or boss
  fight opens with its name; a won fight says "Victory" over the rewards; the run's end is a
  full-screen Defeat/Victory beat (what killed you, the floor) before the end screen.
- **Run end**: defeat/victory with the run's story, retry and new seed in gold, the lists.
- **First run**: a one-screen "How to play" over Neow, shown once.
- **Back**: Android's Back closes a sheet, cancels an aimed card or steps back a room, else opens
  the menu; it never leaves a run.
- **Landscape tablets** (≥ 900 px wide): the field at the left, the hand at the right.
- **Sound**: `sound.js` synthesizes every effect with WebAudio (no files): card played, hit, hurt, block,
  gold, your turn, victory, defeat. On at low volume by default; audio unlocks on the first tap.
- **Settings** (start screen or run menu): theme (dark, light, the phone's), sound on/off and volume, motion
  (the phone's, full, reduced), text size (normal, large), End Turn asking first while energy and a playable
  card are left (off by default), **Keep screen on** (a Wake Lock held while a run is on screen, released on the start and end screens and when hidden, taken again on return; shown only where the browser has it) and **Vibration** (light ticks on a card or tap, when a drag locks a target, and on End Turn, plus the buzz on damage; shown only where `navigator.vibrate` exists, so not on iOS), and this run's seed and ascension. Kept in localStorage.
  **Update toast**: when a new service worker takes control of a page that began under one, "Updated — tap to reload" appears; a tap reloads (never mid-drag). Not shown on the first install.
  **Install app**: Chrome/Android's `beforeinstallprompt` is caught early; the start screen then shows an "Install app" chip that calls `prompt()`, hidden once installed or standalone. On iOS Safari the chip opens a sheet saying "Share → Add to Home Screen".
- **Low-end phones**: looping glows animate opacity or transform only; `lite` (≤ 4 cores or
  ≤ 2 GB, or `?lite`) also drops the bars' blur and the idle loops.

## Upgrade preview, history, daily, share
- **Upgrade preview** (`preview.js`, `Web.upgrade_preview(art, level)`): hold a card (its sheet shows
  the card now and upgraded side by side, changed words lit, old ones struck) or hover / right-click
  it on a desktop (a popover). Works on every card tile: reward, shop, smith, deck, event choice.
- **History** (`history.js`, localStorage `sts2-history-v1`): every won or lost run with character,
  ascension, seed, floor, act, killer, deck, relics, path, date and the save's action log
  (`Web.view().summary` carries the end-of-run data). The start page's History screen shows totals,
  win rate, best streak, per hero and per ascension, top killers and the runs; tap one for its deck,
  relics and path. **Replay hook:** the run detail's "Watch the bot play this seed" calls
  `window.sts2.watchSeed({seed, character, ascension, actions, source})` if it is set, else fires the
  cancelable `sts2:watch-seed` event on `document` (`preventDefault()` to handle it); with neither
  it says the viewer is not available yet.
- **Daily** (`daily.js`): the Daily button plays `dailySeed()`, a hash of the UTC date, with the
  chosen hero and ascension; its result is flagged in the history.
- **Share** (`share.js`): `/?seed=…&char=…&asc=…` prefills the start screen (`char` is the engine's
  index). "Share seed" (start screen, settings, run detail) uses the phone's share sheet, else the
  clipboard.

## The bot (WEB10)
The honest bot (`sts2-diff`'s `honest`, `docs/specs/honesty.md`) runs in the browser, in a Web
Worker (`bot-worker.js`, `worker.js`), from its own wasm: `sts2-web-bot`, built by `build.sh`
into `www/pkg-bot` and fetched on first use (9.9 MB raw, 7.2 MB gzipped: the value, playout and
run-policy nets are `include_bytes`; the service worker keeps it after). It reads only what the
player sees, as in `sts2-diff bot`; `the_web_bot_and_its_reasons_do_not_read_hidden_state` in
`sts2-diff/tests/it/honest.rs` gates it.
- **Ask the bot** (`bot.js`): the **Hint** button in the dock of a fight (beside End Turn) or a choice (Neow, map,
  rewards, card picks, shop, rest, events, treasure). The bot's move is lit on the page (a card and
  the enemy it aims at, a row, a room, End Turn, a potion in the belt; a move of several taps, such
  as a card reward then its card, lights each tap in turn; a shop item is lit directly) and said in one line with why: a
  fight's win chance (the value net on the position) or a choice's confidence (the run policy's
  probability). Fights are the champion's search (value leaves, RLB14's playout blend in boss
  fights) at `advice::PHONE`'s worlds (4/4/12 for 16/16/64); choices the builtin run policy.
  Timing (worker time a move, `proof=botrun`: the bot plays W3B's Defect to victory through the
  page's lit buttons): laptop p50 31 ms, p90 143 ms, boss max 360 ms.
  Settings → **Bot hints → Always show** (off by default) asks in the background at every decision
  and only outlines the move in gold (no text, nothing moves); **Hint** then adds the line.
- **Watch the bot** (`replay.js`): on the run's end screen, and for the last three finished runs on
  the start screen (`sts2.watch(run)`), the bot plays the same seed and hero step by step, drawn by
  the page (read only) with play/pause, Step, speed (1×, 2×, 4×, ½×) and exit (Space, →, Esc on a
  keyboard). **Where you differed** lists both runs' ends, the first split (the first position
  where the bot chose otherwise, a fight's move included) and every run choice of yours the bot
  would have made otherwise, with its reason.
- **Draw odds** (`odds.js`): the fight's draw pile sheet starts with the chance each card (and
  each type) is in next turn's draw of 5 (the base draw; relics and powers that change it are not
  counted) and the cards left before the reshuffle, from the pile's contents, never its order.

`proof.sh`'s real-time bot scenarios run through `cdp.py` (stdlib DevTools client: the worker
needs real time): `botask`, `botchoice`, `botmap`, `botshop`, `botodds`, `botrun`, `botwatch`,
`botwatchplay`, `botstart`.

## Combat numbers, feedback and practice (WEB11)
`www/combatfx.js` (+ `combatfx.css`) holds all of it; `src/fx.rs` is the engine side.
- **Exact numbers**: `Web.preview(card, target)` plays the card on a copy of the fight and returns the deltas
  (damage, Block taken, hits, lethal, your Block and HP, and the enemies' turn after it). While a card is
  selected or dragged each lit enemy shows `18 dmg ×3 · 4 blocked`, you show `+5 block`, the aim bar says
  `End turn after this −3 HP → 63`, and End Turn always carries the HP it would cost and what is left.
- **Feedback**: heals float like damage and Block (all floats ≈0.55 s); End Turn waits 260 ms while the
  enemies wind up (red ring for attackers, gold for the rest). Reduced motion (or the proof) skips the wind-up.
- **Practice**: a switch on the start screen (`?practice` too). Only practice runs show Undo, which takes back the last
  card or potion of this turn (`fx::Undo` keeps the run before each play). Anything that showed something new
  (draw, shuffle, random number, changed intent, a choice from the draw pile) or End Turn clears it. Saves
  carry `practice`; normal runs say `practice: false` and never undo. After a reload the turn's earlier plays are not undoable.
- Proof: `PRACTICE=1 proof.sh preview undo`.

## Saves
After every tap the run (character, seed, ascension and the action list) is saved to
localStorage; Continue on the start screen replays it. The save goes when the run ends.

## Size
`build.sh` uses the workspace's `web` profile (`opt-level = "s"`, fat LTO, `panic = "abort"`) and
runs `wasm-opt -Oz` when binaryen is on PATH (3.17 → 2.87 MB raw). `serve.sh` runs `serve.py`, a
stdlib static server that gzips wasm, js, css and json (the wasm goes as 0.78 MB). No root needed: unpack a binaryen release from
github.com/WebAssembly/binaryen/releases and copy `bin/wasm-opt` to `~/.local/bin` and `lib/*` to
`~/.local/lib`.

## Headless proof
`crates/sts2-web/proof.sh [scenario...]` serves `www/` on localhost and plays each scenario of
`www/proof.js` in headless Chrome by clicking the page's own buttons, writing
`$OUT/<scenario>.png` (`target/web1` by default) and the outcome to `$OUT/titles.txt` (`SEED`,
`CHAR`, `THEME=light`, `SIZE=360x740` pick the run, theme and viewport). `run` plays until the run
ends; `feel` plays a card with animation on (outcome `fly:hit:spent`); `perf` draws 5 enemies and 10
cards with one aimed and times the redraw and the frames (`render=…:frames=p50/p95/max`);
`route` (touch-taps a room two rows up: rooms lit, label), `routetap` (preview, then travel: `true:true`), `pile`, `relic`, `offer`, `shoprelics`, `shopbuy` (buys a card and a potion, each tap asking then buying: `true:true:<gold before>><after>:true`), `act2map`, `act2fight` show WEB5's sheets and act 2. Without
`SEED`/`CHAR` each scenario plays a run that reaches its screen (`seed_for` in `proof.sh`: PROOF
dies early, so `choice` and `treasure` play Necrobinder on W3B, `elite`, `boss` and act 2 the Defect
on W3B). `perf` times in real time (virtual time stops the clock): the page reports its title to
`serve.py`'s log. Every scenario runs through `cdp.py`, which sets the true `SIZE` viewport over the
DevTools protocol (`--window-size` cannot go below about 500 px on macOS; 3x, mobile, touch) and
finds Chrome (`google-chrome`/`chromium` on PATH, else the macOS app); the others run on 20 s of
virtual time, the bot's and `perf` in real time. `drag`
(outcome `<attack hit>:<skill played>:<drag back kept>`), `dragging`, `settings` and `confirm` show WEB6's; `hold`, `enemy`, `turn`, `won`, `dead`, `elite`, `boss` and `tutorial` show the reading sheets, the turn banner and the
first-run help. `TEXT=large` picks the text size.
WEB2's to WEB7's before/after shots are `target/web{2,…,7}/{before,after}`; WEB7's review is
`target/web7/review.md`.

## Real-input test (`realtest.sh`)
    crates/sts2-web/build.sh && crates/sts2-web/realtest.sh     # about 8 minutes; exits non-zero on a failure

`deploy.sh` runs it for you; run it by hand (`WWW=<dir>` picks the directory served) to test a working tree. One memory-capped headless Chrome (`systemd-run`, 4 GB; `ID`, `PORT`, `CDP_PORT`, `HOST`
and `BUDGET` set the unit prefix, the server port, the DevTools port, the LAN address and the seconds per viewport) loads
the real page from the LAN address (not `?proof=`: `body.proof` turns the animations off) and plays whole runs through
CDP input: a finger (touch events) at 390×664 and a mouse and wheel at 1440×900, the two in parallel, each in its own
browser context. `realtest/realtest.py` plays seeds until it has been through a fight (tap or drag a card, target,
End Turn, hover, hold, enemy sheet, draw/discard piles), map, rewards, card pick, event, rest, shop, deck/map/menu/settings
sheets, and a reload mid-run + Continue; a walk that dies starts the next seed.
- After every action the DOM is compared with the engine's view (`window.sts2.view()`): HP, block, powers, intents,
  energy, hand, pile counts, gold, floor, and every creature alive, on screen and not covered, summons and Osty included.
- Enemies, the player row, the field and the hand must not move when a card is tapped, armed or played (once the
  entrance, deal and hit animations have finished; skipped when an enemy died or arrived).
- Every scrollable area reaches its end: touchpad wheel (4 px at 60 Hz), mouse notch (100 px), a swipe on the phone.
- `target/web32/results.txt` has one line per check (`PASS`/`FAIL`), `target/web32/*.png` the screenshots (failures
  and one per screen). A coverage line fails when a screen was never reached.
`drv.py` is the CDP driver (mouse, touch, wheel, keys, screenshots).

## Home screen app, HTTPS and play away from home (WEB43)

The page is installable (manifest, `apple-mobile-web-app-*` tags, the game's logo as the icon, `viewport-fit=cover` and
safe-area padding top and bottom). iPhone Safari has no `navigator.vibrate`, so a tap's feedback is the press dimming
(`platform.css`, every button, card and room: no movement) and the click sound. Plain-http LAN keeps working; the
service worker, offline play and the real clipboard (bug report) need a secure context, i.e. HTTPS.

**Owner, once** (needs you; a worker cannot sign in):
1. On this machine: `sudo tailscale up` and sign in. On the iPhone: install Tailscale, sign in to the same tailnet, switch it on.
2. In the Tailscale admin console (DNS page) turn on MagicDNS and HTTPS Certificates.
3. `sudo tailscale set --operator=$USER` (so `serve` needs no sudo), then
   `tailscale serve --bg --https=443 http://127.0.0.1:8765` (the `sts2-web` unit stays as it is; this only fronts it).
   `tailscale serve status` prints the address, `https://<machine>.<tailnet>.ts.net/`. Undo: `tailscale serve --https=443 off`.
4. iPhone, Safari (not Chrome): open that address, tap Share, Add to Home Screen, Add. Open it from the new icon: no Safari bars,
   the page fills the screen under the notch and home bar. Play once online; after that it opens with no connection.

Not confirmable without the phone: the status-bar look and safe areas on real hardware, that iOS keeps the cache between launches
(iOS may evict a PWA's storage after weeks unused), and that sound unlocks from the home-screen app.

**Service worker.** `/` is network first (3 s, then the cached copy); the page names its build (`<base href="/v/<build>/">`) and the worker
caches that build's shell whole, then art and sounds on demand as they are asked for (not all at once). On the first visit the worker
fetches `/` itself once active, so offline works after one load. A new build reaches a tab on its next load or reload; an open tab keeps
running its own build, whose files come from its cache (never the new build's). The two newest builds stay cached, older ones go.
The "Updated" toast shows only when `sw.js` itself changes.
`sw-check.sh` runs all of this on `http://localhost` (a secure context): offline reload, new build under an open tab, pruning, clipboard, press feedback.
