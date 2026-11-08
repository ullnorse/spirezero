# Changelog

A short record of what changed, newest first. Details live in git, `devlog.md`, the dated
`history/` pages and the task briefs; this page is for a reader who needs the shape of the work
without reading every commit.

## 2026-11-03

### Phone web client (WEB28–31)

- WEB28: `proof.sh` sets the viewport over DevTools, so proofs render at true phone sizes on macOS
  (headless Chrome there would not go below ~500 px).
- WEB29: the phone fight uses its height: enemies show their full picture, the hand is compact
  tiles, unplayable cards dim harder, intents have a colour and icon per kind.
- WEB30: the shop's whole stock on one screen with prices (`Ui::set_flat_shop`, additive in
  `sts2-view`; TUI and Godot keep the two-level shop); card rewards as tiles; rest, treasure and
  Neow show more in their space.
- WEB31: swipe down closes or folds sheets, screen wake lock, haptic ticks, an "Updated" toast and
  an Install button (`www/platform.js`).

## 2026-10-28 to 2026-11-02

### Godot browser build (GD1–GD9)

- A new desktop client that looks like the real game: our Godot 4 project draws the game's own
  scenes, textures, shaders, fonts and Spine skeletons, imported at build time from the owner's
  install, and plays through the sts2-web wasm engine (`specs/godot-frontend.md`).
- GD1 spike; GD2–GD3 the fight plays like the game and matches the owner's recording; GD4 menus,
  Neow, map, events, rewards, shop, rest, treasure; GD5 bot hints and practice undo; GD6 and GD9
  the game's sound, music and ambience; GD7 map tools, shop room, game over, settings, pause,
  abandon confirm, hints on every screen, the game's Proceed steps; GD8 a 27.5 MB start pack with
  per-act and per-hero packs, cached.
- FIX1: tests for choices with an empty hand or an empty pile.

### Phone web client (WEB1–27)

- WEB1–7: the mobile-first page over `sts2-view`: every screen from the model, real game text,
  saves, PWA, rarity frames, animations, drag to play, sound, settings, gzip serving and QR.
- WEB9–13: full relic text, exact card previews, upgrade preview, run history and stats, daily
  seed, share links, a wider map with a route planner, and the bot in wasm (ask the bot, watch it
  play your seed, draw odds).
- WEB14–27: a stable, fluid phone fight layout, drag and redraw fixes, tap the map, active
  indicators for cards and relics (`RelicKind::status`), relic pictures in rewards, whole upgraded
  cards at the smith, bot hints at the thumb.

### TUI and `sts2-view`

- Stages 2–5 of the split: the fight's input moved to `sts2_view::ui::Ui`; the fight, rooms, map,
  shop, menus, launch screens and detail text are all built from `Screen`. UX11–14: a redesigned
  map and a layout walk.

### Bot (honest A0 Ironclad)

- RLB9 78.5% (Waterfall Giant scoring fix) → RLB11 88.0% (combat value net, GPU) → RLB12 90.0%
  (wider value-leaf search) → **RLB14 93.5% test, 92.5% dev** (a learned whole-fight playout
  blended into boss leaves).
- Flat or negative, kept behind flags: RLB10 lookahead, RLB13 AlphaZero tree, VN1 value refit,
  RLB15 playout variants, RLB16 loss anatomy (two thirds of losses at the act-3 boss; a hard
  fight benchmark), RLB17 expert iteration, RLB18 deck probe, RLB19 more act-3 boss search.

## 2026-10-27

### Parity and the referee

- **RF2**: the referee publishes a card/bundle prompt only after it is ready, and a faulted
  background game task is now a command error. This fixed a prompt race that could lose an event
  card selection under load.
- **Reward sets that do not fit**: the referee installs the game's `RewardsSet.testSelector`
  (`referee/tracer/Rewards.cs`): it takes what fits and skips the rest. 56 run loadouts had been
  passing on a dead task; they now compare the real skip. The engine and diff session no longer
  model the dead purchase (`abandon_relic_purchase`, `stuck_event`, `free_purchase_died`).
- **Full matrix 2026-10-27**: all 20 character/act cells 200/200, 0 divergences; regress
  6,175/6,175, regress_runs 2,550/2,550, 3,000 history runs with 0 engine mismatches, coverage 0
  missing. `docs/history/signoff-2026-10-27.md`.
- **Token-potion bug**: `PetrifiedToad`'s `POTION_SHAPED_ROCK` in the belt made
  `TheFutureOfPotions` throw `UnsupportedPotion` and could stick a run. Token potions now behave
  like Common for that event; test `the_future_of_potions_handles_a_token_potion`.
- **Oracle surface**: `sts2-diff oracle-gaps` lists 14 behaviours the referee cannot check; this
  is the documented boundary of the parity claim.

### RL

- **Baseline**: frozen train/dev/test seeds, the observation/action contract, reward and
  evaluation in `docs/specs/a10-training.md`; `python/a10_baseline.py` measures 0/1,000 A10 wins
  with random and greedy-fight play; 124.7k decisions/s random, 6.9k greedy.
- **First combat learner**: `python/train_combat_ppo.py` (masked PPO, 256×2 MLP) reached **0.770**
  win rate vs **0.329** random on 512 W1-all combat episodes; ~66k train steps/s.
- **A0 positive control (clairvoyant)**: `sts2-diff reach` won Ironclad `R35W61N88ZUQ` at A0 and
  replayed to victory. RLB5 later tuned the same clairvoyant search to 19.0% A0 Ironclad
  (38/200). These are **upper bounds**, not the honest bot.
- **Honesty**: `docs/specs/honesty.md` defines visible vs hidden and the action-leak gate.
  `Combat::blindfold_for_search(seed)` resamples the hidden draw/discard order and RNG for search
  worlds; the observation-leak test covers it. The combat PPO is honest (observation-only); the
  honest run policy is the next piece.
- **Environment fixes**: `final_obs` keeps the ended episode's observation for terminal HP/floor
  shaping (ABI 3); the `PETRIFIED_TOAD` token-potion bug above.

### Clients

- **`sts2-view` stage 1**: pure text (markup → `Role`-tagged `Rich`), names, descriptions and
  out-of-combat menus moved out of the TUI. The TUI loads localization and maps roles to ratatui;
  names/describe/menu are re-exports; no screen changed.
- **Stage 2a**: `Screen`, `Block`, `Button`, `ButtonId`, `Input`, sheets and pages with serde/JSON
  round-trip; the contract for the iOS ABI.
- **Stage 2b**: a pure `combat::screen` builder (enemies, intents/powers, player/pets/orbs,
  incoming, hand rows with engine legality and card text, piles, End Turn). The TUI renderer
  switch and a shared `Ui` are next.
- **Gate**: `sts2-core` and `sts2-view` are checked for `wasm32-unknown-unknown`; docs links and
  entry points are checked in the gate.
- **iOS**: the Rust crates are ready to cross-compile; the SwiftUI app waits for Xcode. Personal
  use only (game assets/text).

### TUI

- UX rounds 5–10 and the FIX1 hover/Enter fix are merged: phone-style one-column screens, row
  buttons, one-key potions, the big-screen scaling, full card text, the last-turn line, paged
  lists and the in-place confirmations.
- The next TUI work is the renderer switch to `Screen`/`Input` (stage 2b), not more layout in
  `app.rs`.

### Repo

- Docs got one entry point (`docs/README.md`), one current-state page (`docs/STATUS.md`), a
  `history/` folder for dated records and a `regression.md` for the verification stack.
- Finished briefs moved to `tasks/done/`; the backlog was curated; `cargo xtask status` and
  `merge-check` exist; `cargo xtask docs-check` runs in the gate.
- All merged worktrees and branches were removed; `task/RL10`, `task/RLB5` (diagnostic) and
  `task/S4A` remain parked.
- Behaviour coverage: hits files record played cards, used potions and changed powers;
  `tools/hits.sh --effects` shows 497/558 cards, 64/64 potions, 231/242 powers exercised by the
  run loadouts.

## 2026-10-23 to 2026-10-25 (after the recovery)

- The disk recovery squash and the referee rebuild path (`docs/history/RECOVERY-2026-10-23.md`).
- Act 1 signed off 2026-10-18; Hive and Glory content complete; all five characters ported.
- SG1/SG2 sign-offs, PX27–PX30 parity fixes and RF1's event-fight reward handling;
  `docs/history/signoff-2026-10-24.md`, `signoff-2026-10-25.md`.
- TUI rounds 1–4 and the client direction (`docs/specs/ui-reimagined.md`).
- RL: the frozen-seed observation/action contract and the A10 baseline (`a10-training.md`).

## Before 2026-10-23

- **Engine**: project start 2026-10-16 on Necrobinder; its Act 1 matched the referee on 2026-10-17;
  all five characters' Act 1 was signed off on 2026-10-18 after 26,000 runs (one real divergence:
  a Fairy in a Bottle in an event).
- **Verification**: the headless referee, the differential harness, exact C# decimal/RNG/shuffle
  parity, community-run history replay, entity coverage and the panic fuzz.
