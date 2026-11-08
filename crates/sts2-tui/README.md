# sts2-tui

A terminal UI that plays a whole run, Neow to the victory screen (Acts 1–3, ascension 0–10, the
second Act 3 boss at A10), for any of the five characters on the Rust engine: the ancients, the
map, every room type, events, rewards, shops, rest sites and the bosses. It uses only the public
`Run` API; what you can choose is whatever `Run::legal_actions` allows.

```
cargo run -p sts2-tui -- [SEED] [--character NAME] [--ascension N] [--no-mouse]
```

With no arguments the main menu opens when a run is saved (Continue, New run, Settings, Quit),
else the character select (character, ascension, optional seed). With any argument the run starts
at once (Necrobinder by default, a random seed unless one is given). The act, the map and
everything else come from the seed's own roll, as in the game.

## Playing

The screen is one decision at a time, and every row prints the key that chooses it. `Enter` is
the screen's main button (play, take, start, continue), `Esc` goes back or closes, `?` lists the
keys and mouse actions of the screen shown (the help and the input handlers read the same table,
`keys.rs`), and `q` quits at once (the run is saved). In a fight the hand is a list of numbered
rows; the numbers select and play, `e` ends the turn, and the potions on the belt print their own
keys (`z c b n g`). The keys are a fallback: everything can be clicked too, with the mouse wheel
for long lists.

- **`w` asks the bot** what it would do at the decision on show: the honest bot of the web's
  "ask the bot" (`sts2_diff::honest`, the champion at the phone's search size; it sees only what
  you see, `docs/specs/honesty.md`). It runs on a thread of its own: the line under the choices
  says `Bot: thinking…` and the keys keep working; the answer names the row or card to press and
  how sure the bot is (`Bot: play 3 Strike on 2 Hexaghost (Win chance 71%)`). It goes away with
  your next action. The bot's nets are built on the first ask.
- **Details.** A `›` on a row means it has details: right click, or `i` then its number, opens
  them; on a terminal 150 cells wide or more the panel at the column's right shows them for the
  highlighted row (cards, choices, and now the next rooms of the map: what the room is and how many
  rests, elites, shops and so on every path through it still passes). Hovering a potion or a relic
  of the top bar shows its text in the panel too (right click opens it as a sheet).

The screen direction is `docs/specs/ui-reimagined.md`; the rounds that produced the current TUI
are recorded in `docs/history/tui-ux-next.md`. Snapshots of the screens at 45×30, 80×24, 120×35,
160×45 and 200×55 are in `tests/snapshots`; `cargo test -p sts2-tui --test it` plays whole runs,
mouse flows and snapshots headlessly.

### What changed in UX16

Played at 80x24 and 160x45: the map header and legend keep the counts of rooms ahead at 80x24;
an event's story is no longer cut while there is a blank row to give; enemies' powers do not run
into each other and the ones that do not fit are counted (`+2`); the card reward's Skip row does
not list the cards; a shop section with nothing to buy says why; the panel is there at 160x45.

## Session

- **Autosave and Continue.** After every action the run is written as its seed, character,
  ascension and action list (each action as its index in `App::recordable_actions`; a few KB for
  a whole run) with a digest of the state. The next launch opens on the main menu with
  **Continue**; `Enter` replays the list and checks the digest (a save from another build
  continues with a warning if the digest differs, or says it cannot be continued if an action no
  longer applies). A finished run deletes the save.
- **Where.** `$SPIREZERO_HOME` if set (one directory for everything), else Linux
  `~/.local/share/spirezero/save.txt` and `~/.config/spirezero/settings.toml`
  (`$XDG_DATA_HOME`, `$XDG_CONFIG_HOME`), macOS `~/Library/Application Support/spirezero/`,
  Windows `%APPDATA%\SpireZero\`. Writes go through a temporary file.
- **Settings** (`o`, saved at once; `settings.toml` can be edited by hand, a bad value means the
  default): colours (`dark`/`light`/`muted`), glyphs (Unicode or ASCII), mouse, the rewards
  confirmation, what the boss key shows (`build` log or `top`), the boss keys (file only), and the
  last character and ascension. **Boss key** (`` ` `` or `F12`) turns the screen into a plain log
  or process list; any key brings the game back.
- **Crashes.** A panic restores the terminal, writes `crash.txt` beside the save and prints where
  it is; the next launch offers Continue as usual plus `b` (continue from before the last
  action). Replay runs under `catch_unwind`, so a save that crashes the engine shows an error.

## Where the text comes from

Card, relic, potion, power and event text is the game's English localization. It is **not** in
this repository. At start-up the TUI reads the localization `.json` files from the first of:

1. `$STS2_LOCALIZATION`
2. `referee/vendor/sts2-cli/localization_eng` (gitignored)
3. `referee/vendor/upstream/localization_eng` (gitignored)
4. `./localization_eng`

A linked git worktree has no `referee/vendor`; point `$STS2_LOCALIZATION` at the main checkout's
copy there. Without the files it shows engine ids and names only, and everything still works;
tests use small synthetic tables.

Placeholders are filled from the engine, never from the UI's own arithmetic: card previews from
`CombatState::card_preview` / `RunCard::preview` (with the hooks for a hand card), powers and
relics from their `vars`, events from `EventRoom::vars`, enchantment titles from the engine. A
few vars the engine does not keep (Punch Off's rolled gold, Wongo points, Trial's story, the Tea
Master's tea text, Battleworn Dummy's HP settings, Colossal Flower's prizes) stay as `‹Name›`;
a finished page with no options shows no body.
