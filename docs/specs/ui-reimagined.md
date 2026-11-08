# Slay the Spire 2 as a mobile game in a terminal

Status: design (UD1, 2026-10-25). Supersedes `tui-clean.md` and `tui-redesign.md`. The UX5
branch (narrow column, top bar, row buttons, hand as a list) is the first prototype of this
direction; this spec keeps its good parts and settles the rest.

What the user asked for: the game's mechanics, exactly, with a UI that is "super simple", made
as if it were a mobile game that happens to run in a terminal. The phone *is* a target today:
the user plays over SSH from a phone (Termius), a portrait terminal of about 44×30 cells. Later
a real mobile client reuses everything but the drawing.

Non-negotiable: numbers, choices, randomness and hidden information are the engine's
(`sts2-core`), untouched. Only presentation and interaction change. Everything the real game
shows somewhere (intents, powers, keyword text, piles, relic text) stays reachable, one tap away.

## 1. Product principles

1. **One column, one decision.** Every screen is a portrait column, at most 44 cells wide, that
   asks one question: which card, which target, which option, which room. What is needed to
   answer that question is on the screen; everything else is one tap away. A wide terminal
   centres the same column and uses the spare width for a detail panel, never for more things.
2. **Tap first, keys as printed shortcuts.** Every action is a full-width row button. A button
   prints its key at the left (`1`–`9`, `0`, `e`, `Enter` for the screen's main button, `Esc` for
   back). No chords and no digit modes (`p` then `1`, `i` then `3` go away): keys are whatever
   the screen prints, so the help is the screen itself. Two-digit numbers never exist: a list
   longer than ten rows is paged, and the page's rows are numbered 1–0.
3. **Details on demand, without hover.** Every row that stands for a game object (a card, an
   enemy, a power, a relic, a potion, a reward, a map room) ends with `›`. Tapping `›` (or the
   `i` key followed by the row's number, or a right click on the row) opens the **detail sheet**:
   the object's full game text with its numbers filled in, its keywords explained, an enemy's
   intent in words and every power with its text. This replaces hover tooltips. On a wide
   terminal the sheet is a permanent right-hand panel showing the last thing pointed at.
4. **Information hierarchy per decision.** Combat: what they will do to me, what I can play,
   what it does (the engine's preview), then my resources. Map: what rooms are next, then the
   rest of the map. Rewards and shops: what I get, what it costs, what I have. The top bar is
   the same everywhere and is the only thing always on screen.
5. **Numbers come resolved from the engine.** Card text uses `CombatState::card_preview`,
   intents `intents()`, incoming damage `incoming()`, playability `unplayable_reason()`. The UI
   layer never adds, subtracts or guesses.
6. **Irreversible choices confirm in place.** A dangerous row (skip a reward, remove a card,
   leave a shop with rewards unclaimed, abandon a run) turns into `Tap again to confirm` for one
   more tap. No modal dialogs.
7. **Colour is a second channel, never the only one.** Roles, not colours, are in the model
   (HP, gold, energy, attack intent, debuff, keyword, dim). Each renderer maps roles to its
   palette; a muted theme and an ASCII glyph set must stay readable.
8. **Nothing decorative.** No pictures, no boxes around single lines, no frames except the one
   around the current list and the double frame around the screen's main button (UX5's
   convention). Rules (`───`) separate sections.

### Shared chrome

```
 ♥ 52/66   ◉ 99   Floor 2                 ☰
 ───────────────────────────────────────────
 ◈ Fire  ◈ Block
```

The top bar: HP, gold, floor, and `☰` (the run menu, key `m`). Potions on a row of their own
when there are any; a potion row is a button (sheet: text, `Use`, `Throw away`). Act and deck
count, relics, seed and ascension live in the run menu, not on the bar. A character resource
joins the bar only when it exists: `✦ 3` stars for the Regent (always, as the game's
`ShouldAlwaysShowStarCounter`) and for others when above zero.

Keys that are the same on every screen: `Esc` back/close, `Enter` the main button, `m` menu,
`i`+digit detail, `?` help (a sheet listing exactly these), `PgUp`/`PgDn` or the wheel to page a
list, `q` quit from the menu only. Digits and letters are whatever the rows print.

## 2. Screens

Every mockup is 44 cells wide. "Wide" says what changes at 80 and 120 columns.

### 2.1 Launch and main menu

Purpose: continue or start. Shows the saved run's one-line summary if there is one, and the
crash note if the last session crashed.

```
 SLAY THE SPIRE 2             SpireZero 0.1

 Saved: Ironclad · Act 1 · Floor 7 · ♥ 48/80

 ╔════════════════════════════════════════╗
 ║  Enter  Continue                       ║
 ╚════════════════════════════════════════╝
 ┌────────────────────────────────────────┐
 │ 1  Continue from the step before       │
 │ 2  New run                             │
 │ 3  Settings                            │
 │ 4  Quit                                │
 └────────────────────────────────────────┘
```

Wide: centred, unchanged.

### 2.2 Character and ascension select

Purpose: pick a character, an ascension, optionally a seed, start. One screen: the character
rows are the choice, the selected character's text is under them (the game's card: HP, gold,
starter relic with its text), then the ascension stepper and the seed row.

```
 NEW RUN                                  ☰
 ───────────────────────────────────────────
 ┌────────────────────────────────────────┐
 │ 1  Ironclad                      ♥ 80  ›│
 │ 2  Silent                        ♥ 70  ›│
 │ 3  Regent                        ♥ 75  ›│
 │ 4  Necrobinder                   ♥ 66  ›│
 │ 5  Defect                        ♥ 75  ›│
 └────────────────────────────────────────┘
 IRONCLAD  ♥ 80/80  ◉ 99
 The last soldier of the Ironclads.
 Burning Blood: At the end of combat, heal
 6 HP.
 ┌────────────────────────────────────────┐
 │ -  Ascension 10  +   Double Boss       ›│
 │ s  Seed  7MBWSKU4ZAQH          random  │
 └────────────────────────────────────────┘
 ╔════════════════════════════════════════╗
 ║  Enter  Start                          ║
 ╚════════════════════════════════════════╝
```

Actions: a character row selects it (the selected row is highlighted); `-`/`+` (or taps on the
ends of the row) step the ascension and the row's text is that level's rule; `›` on it lists all
levels' rules; `s` edits the seed (typing replaces it, `Esc` restores random). The order of the
rows is the game's select-screen order (`SELECT_ORDER`).

Wide: the character's text moves to the right panel; the rows stay.

### 2.3 Neow and the act's ancient

Same screen as an event (2.7): title, the ancient's line, option rows with the game's text.
Options that cost something show the cost in the row (`Lose all Gold.`) because the text does.

### 2.4 Map and path choice

Purpose: pick the next room. The decision rows come first; the map is context and scrolls.

```
 ♥ 52/66   ◉ 99   Floor 2                 ☰
 ───────────────────────────────────────────
 ◈ Fire
 NEXT ROOM
 ┌────────────────────────────────────────┐
 │ 1  ⚔ Monster                           ›│
 │ 2  ? Unknown                           ›│
 └────────────────────────────────────────┘
 AHEAD  4 rests · 1 shop · 2 elites · boss
      ┌─ 15 ────────────── BOSS ──────────┐
   14    R     E        R        M
   13  M    ?     $        M        M
   12    M     M     M        ?
    ⋮
    4     M     ?     M        ?     M
    3  [1]M    M     ?        ?        M
    2     ●         M        M        M
    1               ⚔
 Legend  M monster  E elite  ? unknown
         $ shop  R rest  T treasure  ● you
```

The map is a compact grid: one row per floor, one cell per column; the paths are implied by
adjacency and the reachable next rooms carry their number (`[1]`). `PgUp`/`PgDn` or the wheel
scrolls the floors; the rows with the choice stay. `›` on a next-room row shows what is known
about the room (type, and for the boss its name). The full-screen map (menu entry) is the same
grid with the edge lines drawn, for reading the whole act.

Wide (≥ 100): the column keeps the rows and the right panel holds the full map with edges.

### 2.5 Combat

Purpose: play a card at a target, use a potion, or end the turn. Reading order top to bottom:
enemies (number, name, intent, HP, statuses), your side (HP, Block, incoming, pets, orbs),
the hand, piles, End Turn.

```
 ♥ 52/66   ◉ 99   Floor 2   ✦ 3           ☰
 ───────────────────────────────────────────
 ◈ Fire  ◈ Block
 1 Leaf Slime (S)                         ›
   ⚔ 3     ████████░░░░ 8/14   Weak 1
 2 Leaf Slime (M)                         ›
   ✚ Slimed 2  ████████████ 33/33
 3 Twig Slime (S)                         ›
   ⚔ 4     ████████████ 7/7    ⛨ 6
 ───────────────────────────────────────────
 You  ████████░░░ 52/66   ⛨ 5   −7       ›
 Osty 1/1  Die 1                          ›
 ───────────────────────────────────────────
 Round 1 · your turn               ϟ 3/3
 ┌────────────────────────────────────────┐
 │ 1  1 Strike    Deal 6 damage.         ›│
 │ 2  1 Defend    Gain 5 Block.          ›│
 │ 3  1 Bodyguard Summon 5.              ›│
 │ 4  2 Bash      Deal 8. Apply 2 Vuln…  ›│
 │ 5  X Whirlwind Deal 5 damage X times. ›│
 └────────────────────────────────────────┘
 Draw 6 ›  Discard 0 ›  Exhaust 0 ›
 ╔════════════════════════════════════════╗
 ║  e  End Turn                           ║
 ╚════════════════════════════════════════╝
```

- **Enemy block:** two rows each. Row one: number, name, `›`. Row two: intent (the engine's
  resolved damage, `⚔ 6×3` for multi-hit, `✚` buff, `▼` debuff, `⛨` defend, `?` unknown),
  HP bar, Block, then up to two power tags; more powers show as `+2` and all of them are in
  the sheet. Secondary enemies (minions) are listed after primaries, as the engine orders them.
  A dead enemy leaves the list.
- **Your side:** HP bar, Block, the incoming total after Block as `−7` (from `incoming()`),
  `›` opens your powers and relics in play. Pets (Osty) and orbs each take one row when
  present: `Osty 1/1  Die 1`, `◯ Lightning 3 · Frost 2 · [ ] [ ]` (evoke values from the
  engine; `›` explains each orb). Stars sit in the top bar.
- **Hand:** one row per card: key, cost (`X` for X-cost; a changed cost shows the current
  one), name, one line of text, `›`. Unplayable cards are dim and their sheet says why
  (`UnplayableReason`). The selected card's row is highlighted.
- **Play a card:** tap the row (or its key). A card with no target is played at once (with
  the confirmation rule of §1.6 for cards the settings mark as confirmed, none by default). A
  card that needs an enemy enters **targeting**: the enemy rows become the buttons, every
  other row dims, each enemy shows the engine's preview after the card (`→ 2/14`, `☠` lethal),
  `Esc` cancels. The hand's keys are not live while targeting, so `1` is enemy 1.
- **Potions** use the same two steps from the potion row's sheet.
- **End Turn** is the main button (`e`, `Enter`). Ending with playable cards and energy left is
  not confirmed (the game does not either).

Targeting, at 44 wide:

```
 Strike → pick a target             Esc back
 ┌────────────────────────────────────────┐
 │ 1  Leaf Slime (S)   8/14 → 2/14       ›│
 │ 2  Leaf Slime (M)  33/33 → 27/33      ›│
 │ 3  Twig Slime (S)    7/7 → ☠          ›│
 └────────────────────────────────────────┘
```

**Hard cases**
- *10-card hand:* keys `1`–`9`, `0`. At 30 rows, the enemy block compresses to one row per
  enemy (`1 Leaf Slime (S)  ⚔ 3  ████░░ 8/14 ›`) and the hand gets every row it needs; if
  the hand still does not fit, the hand list scrolls with the cursor and the wheel, the frame
  shows `▲`/`▼` marks, and the keys stay on the cards (not the page) because ten keys cover
  ten cards.
- *5 enemies:* one row each from the start (the compact form), powers behind `›`. Names are
  cut with `…` before the intent is.
- *Long power lists:* the row shows two tags; the sheet lists all with the game's text and the
  stack numbers.
- *Multi-step selections:* see 2.6.

Wide (80): the column is centred; the enemy block uses the two-row form. Wide (120): the
column sits left of centre and the right panel shows the detail of the hovered or last `›`
row, so hovering a card or an enemy with the mouse gives the tooltip behaviour people expect
on a desktop without the model knowing about hover.

### 2.6 In-combat choice prompts

The engine's only decision type is `DecisionRequest::ChooseCards` (pick `min..=max` of a list,
one at a time, then `Done`). Every in-combat prompt (discard from hand, exhaust, choose a card
to add from a draw-pile or discover list, scry-like "discard any of these", Gnarled Hammer's
upgrade, Knowledge Demon's curse pick) is the same **choice sheet** over the hand area:

```
 Discard 1 card.  (0 of 1)          Esc —
 ┌────────────────────────────────────────┐
 │ 1  1 Strike     Deal 6 damage.        ›│
 │ 2  1 Defend     Gain 5 Block.         ›│
 │ 3  2 Bash       Deal 8. Apply 2 Vuln… ›│
 └────────────────────────────────────────┘
```

- The title is the engine's prompt text (`Describer::prompt`), with the count `(picked of max)`.
- Tapping a row picks it (`Decision::Pick`); a picked row shows `✓` and stays in the list only
  if the engine allows picking it again. When `picked ≥ min`, the main button reads `Done`
  (`Decision::Done`); when `picked == max` the sheet confirms itself. `Esc` is shown only if
  the engine allows cancel (it does not for combat prompts; `—` marks that).
- X-cost cards do not prompt: the engine spends all energy; the card's sheet says so.
- A choice from a pile shows the pile's cards in the order the engine gives (the draw pile is
  sorted by the engine when the game hides its order).
- Lists longer than ten rows page.

Out-of-combat deck selections (`RunDecision::CardSelect`: smith, remove, transform, enchant)
use the same sheet as a full screen, with `Esc` live when the engine lists `CancelSelect`.
Choose-a-bundle (`RunDecision::Bundles`) is a list of bundles; each bundle's row names its
cards and `›` lists them.

### 2.7 Events

Purpose: read, then choose. Title, the story text (wrapped, the game's markup colours as
roles), then the option rows with the game's option text and the filled numbers.

```
 ♥ 52/66   ◉ 15   Floor 3                 ☰
 ───────────────────────────────────────────
 TRASH HEAP
 You discover a towering pile of scrapped
 weapons, discarded trinkets, and other
 oddities. The massive pile shifts and
 rumbles, as if growing from within...

 If you scavenge from the surface, you may
 find some decent things. But if you really
 get in there, you may find some exotic
 treasures.
 ┌────────────────────────────────────────┐
 │ 1  Dive In                             ›│
 │    Lose 8 HP. Obtain a random forgotten│
 │    Relic from the past.                │
 │ 2  Grab Random Junk                    ›│
 │    Gain 100 Gold. Obtain a random      │
 │    forgotten card from the past.       │
 └────────────────────────────────────────┘
```

A long story scrolls (the rows stay at the bottom). A finished page shows its outcome text and
one `Continue` main button. An option the engine does not list as legal is dim and says why in
its sheet when the text gives a reason.

Wide: unchanged, centred; the text wraps to at most 56 cells for reading.

### 2.8 Rewards

Purpose: take or skip each reward, then proceed. One row per reward, in the engine's order;
a taken reward leaves the list. A card reward row opens the card pick (2.9).

```
 VICTORY
 ┌────────────────────────────────────────┐
 │ 1  ◉ 17 Gold                           │
 │ 2  ◈ Fire Potion                      ›│
 │ 3  Card reward                      3 ›│
 │ 4  ◆ Vajra                            ›│
 └────────────────────────────────────────┘
 ╔════════════════════════════════════════╗
 ║  Enter  Proceed                        ║
 ╚════════════════════════════════════════╝
```

`Proceed` with rewards left confirms in place (`Skip 3 rewards? Tap again`). A full potion belt
shows the potion row dim with `belt full` and its sheet offers to throw a held potion away.

### 2.9 Card pick (rewards, shop previews, Neow's card options)

```
 CHOOSE A CARD                   Esc skip
 ┌────────────────────────────────────────┐
 │ 1  1 Twin Strike  Attack · Common     ›│
 │    Deal 5 damage twice.                │
 │ 2  1 Inferno      Power · Uncommon    ›│
 │    At the start of your turn, deal 3   │
 │    damage to ALL enemies.              │
 │ 3  1 Rupture      Power · Uncommon    ›│
 │    Whenever you lose HP from a card,   │
 │    gain 1 Strength.                    │
 └────────────────────────────────────────┘
 ┌────────────────────────────────────────┐
 │ r  Reroll                     Driftwood│
 │ s  Skip                                │
 └────────────────────────────────────────┘
```

The full text is on the screen because the choice is about the text. Alternatives (`Reroll`,
`Sacrifice`) are rows only when the engine lists them. `Skip` confirms in place.

### 2.10 Shop

Purpose: buy. Two levels: the shop's sections, then a section's items. Gold is in the top bar;
unaffordable items are dim with the price still shown.

```
 MERCHANT                        ◉ 300
 ┌────────────────────────────────────────┐
 │ 1  Cards                            7 ›│
 │ 2  Colorless cards                  2 ›│
 │ 3  Relics                           3 ›│
 │ 4  Potions                          2 ›│
 │ 5  Remove a card                  75 ◉ │
 └────────────────────────────────────────┘
 ╔════════════════════════════════════════╗
 ║  Enter  Leave                          ║
 ╚════════════════════════════════════════╝

 CARDS                           ◉ 300
 ┌────────────────────────────────────────┐
 │ 1  1 Breakthrough  Attack        51 ◉ ›│
 │ 2  3 Mangle        Attack       156 ◉ ›│
 │ 3  2 Impervious    Skill        144 ◉ ›│
 │ 4  1 Shrug It Off  Skill    SALE 25 ◉ ›│
 │ 5  1 Inferno       Power         78 ◉ ›│
 └────────────────────────────────────────┘
```

Buying confirms in place (the row becomes `Buy for 51? Tap again`). Card removal opens the deck
selection (2.6). Leaving does not confirm.

Wide: the sections and the open section's items side by side.

### 2.11 Rest site

```
 REST SITE
 ┌────────────────────────────────────────┐
 │ 1  Rest        Heal 20 HP (to 72/80)   ›│
 │ 2  Smith       Upgrade a card          ›│
 │ 3  Lift        Gain 1 Strength (Girya) ›│
 └────────────────────────────────────────┘
```

Rows are the engine's `RestOption`s with `Describer::rest_option` text and the resulting HP.
Smith opens the deck selection with `Esc` live.

### 2.12 Treasure and boss relics

Treasure: one relic row with its text, a `Take` main button; `Proceed` leaves without it when
the engine allows. Boss relic choice (end of act): a list of the offered relics with their text,
`›` for the full text, pick one; no skip unless the engine lists it.

### 2.13 Deck, relics, potions, piles, log, visited

All from the run menu (`☰`, `m`) or the `›` on `Draw`/`Discard`/`Exhaust`. Each is a paged list
sheet of rows with `›`; `Esc` closes. The deck lists cards in deck order with the upgrade mark
and the enchantment; relics with the counter the engine exposes; the log is the fight's lines,
newest at the bottom, paged.

```
 ☰ MENU                                Esc
 ┌────────────────────────────────────────┐
 │ 1  Deck                             14 │
 │ 2  Relics                            3 │
 │ 3  Map                                 │
 │ 4  Log                                 │
 │ 5  Rooms visited                       │
 │ 6  Settings                            │
 │ 7  Help                                │
 │ 8  Abandon run                         │
 └────────────────────────────────────────┘
 Ironclad · A10 · Act 1 · seed 7MBWSKU4ZAQH
```

### 2.14 The detail sheet

Opens from any `›`. Portrait: covers the lower two thirds of the column, the screen behind stays
visible above. Contents by object:

- Card: cost, name, type, rarity, the full text with live numbers, each keyword's definition
  (`KEYWORDS` order), the enchantment, why it is unplayable.
- Enemy: name, HP, Block, the intent in words (`Attacks for 6, 3 times`, `Buffs itself`,
  status cards it will give), every power with its text and amount.
- Your side: every power on you, relics in play, orbs with passive and evoke values, pets.
- Relic, potion, reward, map room, rest option, event option: the game's text.

```
 ───────────────────────────────────────────
 BASH                  2 ϟ · Attack · Common
 Deal 8 damage. Apply 2 Vulnerable.

 Vulnerable: Target takes 50% more damage
 from attacks.

 In this fight: 8 damage (Strength 2).
 ┌────────────────────────────────────────┐
 │ 1  Play                                │
 │ Esc  Close                             │
 └────────────────────────────────────────┘
```

Wide (≥ 100): the same content lives in the right panel, updated by hover or the last `›`; no
sheet covers the column.

### 2.15 Run end

```
 DEFEAT                     Act 1 · Floor 7
 Slain by Fuzzy Wurm Crawler.

 Ironclad · Ascension 10 · seed 7MBWSKU4ZAQH
 Rooms 7 · Fights 4 · Cards 14 · Relics 3
 ┌────────────────────────────────────────┐
 │ 1  Retry this seed                     │
 │ 2  New seed                            │
 │ 3  Characters                          │
 │ 4  Rooms visited                      ›│
 └────────────────────────────────────────┘
```

Victory is the same screen with the title `VICTORY`.

### 2.16 Settings and help

Settings is a list of rows whose value cycles on tap: theme (dark, light, muted), glyphs
(unicode, ascii), mouse, confirmations, boss view. The art and layout rows go: there are no
pictures and one layout. Help is a sheet of the global keys and the sentence "every other key is
printed on its button".

## 3. Architecture: a presentation layer the TUI and a mobile client share

### 3.1 The split

```
sts2-core ──► sts2-view ──► sts2-tui   (ratatui: draws Screen, maps events to Input)
                       └──► sts2-web   (wasm-bindgen: Screen as JSON, a thin web page)
```

`sts2-view` is a new crate: no ratatui, no crossterm, no file system, no threads. It depends
on `sts2-core` only (plus `serde` behind a feature, for the web). It holds everything that is
not game rules and not drawing:

| Today (`sts2-tui`) | Goes to | As |
|---|---|---|
| `app.rs` (`App`, `Input`, `Overlay`, `Inspect`, replay, visits) | `sts2-view::ui` | `Ui`: the state machine; `Ui::screen()`, `Ui::input()` |
| `menu.rs` (`MenuItem` over `RunAction`) | `sts2-view::screens` | the per-screen builders |
| `describe.rs`, `names.rs` | `sts2-view::text` | unchanged, output as role-tagged spans |
| `text.rs` (localization, markup) | `sts2-view::text` | the markup parser emits `Rich`, no ratatui styles |
| `keys.rs` (layers, bindings) | mostly deleted | the global keys in `Ui::input`; row keys come from the `Screen` |
| `session.rs` save format (`Save::to_text/parse/restore`) | `sts2-view::save` | pure; paths and files stay in the TUI |
| `session.rs` dirs, crash notes, fingerprint | `sts2-tui` | terminal matters |
| `hit.rs` | `sts2-tui` | hit regions map rectangles to `ButtonId`s |
| `settings.rs` | `sts2-tui` (theme, glyphs, mouse, boss) and `sts2-view` (confirmations) | |
| `ui/*` | `sts2-tui` rewritten | one drawing function per `Block` kind |
| `shell.rs` | `sts2-view::ui` (select screen state) and `sts2-tui` (event loop) | |

### 3.2 Types

```rust
// sts2-view
pub struct Screen {
    pub top: Option<TopBar>,            // HP, gold, floor, stars, potions
    pub title: Option<Rich>,
    pub blocks: Vec<Block>,             // in reading order
    pub primary: Option<Button>,        // End Turn, Proceed, Start, Done
    pub sheet: Option<Sheet>,           // detail, choice, or menu over the screen
    pub status: Option<Rich>,           // a transient line: "Not enough energy"
}
pub enum Block {
    Text(Vec<Rich>),                    // story, descriptions
    Enemy(EnemyRow), You(YouRow), Pet(PetRow), Orbs(OrbRow),
    List { frame: bool, rows: Vec<Button>, page: Option<Page> },
    Map(MapGrid),                       // the compact grid; the TUI draws it, the web draws it
    Rule,
}
pub struct Button {
    pub id: ButtonId,                   // stable within one Screen
    pub key: Option<Key>,               // what the row prints: Digit(1), Char('e'), Enter, Esc
    pub label: Rich, pub detail: Vec<Rich>, pub right: Rich,
    pub state: ButtonState,             // Enabled, Dim(reason), Selected, Picked, Disabled
    pub info: Option<InfoRef>,          // the `›`: what the detail sheet shows
    pub confirm: Option<Rich>,          // "Tap again to confirm": second-tap text
}
pub struct Rich(pub Vec<(Role, String)>);   // Role: Plain, Hp, Gold, Energy, Attack, Debuff,
                                            // Buff, Keyword, Dim, Lethal, Preview ...
pub enum Input { Tap(ButtonId), Info(ButtonId), Key(Key), Page(isize), Back, Primary }
```

`Ui::input(Input)` is the only way in. It resolves a `Key` by looking at the current `Screen`
(the row whose `key` matches, else the global table), turns taps into the engine's
`RunAction`s, handles the two-step rules (targeting, confirm in place, pick-then-done) and
calls `Run::apply`. `Ui::screen(&self) -> Screen` is a pure function of `(Run, UiState)` and is
rebuilt after every input; it is small (hundreds of bytes of strings), so no caching.

The renderer's whole job: draw a `Screen` and map its events (key, click, wheel, touch) to
`Input`. It never looks at `Run`.

### 3.3 Why this split

- **Everything with a decision in it is tested once.** `Ui` tests drive `Input`s and assert
  on `Screen`s, with no terminal. The TUI's tests shrink to snapshots of drawing.
- **The mobile client is a renderer, not a port.** It needs the drawing of six block kinds and
  the sheet. The `Screen` is already a list of rows and buttons, which is what a phone UI is.
- **The engine pays nothing.** `sts2-view` reads `sts2-core`'s public API as the TUI does
  today. No new code, trait, feature or dependency in `sts2-core`; `serde` is derived on
  view types only, behind the view crate's `serde` feature. RL and simulation never build it.
- **It is simpler than today.** The key layers (750 lines), the hover and tooltip code, the
  hit-target enum with sixteen variants and the three input modes collapse into "rows have
  keys" and "rows have ids".

### 3.4 The mobile client: wasm plus a thin web page

Recommendation: compile `sts2-core` + `sts2-view` to `wasm32-unknown-unknown` with
`wasm-bindgen` in a small `sts2-web` crate exposing three functions (`new(seed, character,
ascension)`, `screen() -> JSON`, `input(JSON)`, plus `save()/restore(text)`), and render the
JSON with a few hundred lines of plain HTML/JS as a progressive web app.

Why, over the alternatives:

- *Over `sts2-ffi` and native Swift/Kotlin UIs:* that crate is the RL batch ABI, a different
  product; and a C ABI means two native UIs, two build systems, and marshalling `Screen` by
  hand. JSON over wasm is one line each way.
- *Over a Rust UI toolkit (Dioxus, Slint, egui):* a card game's UI is lists, buttons, text and
  one bar; the DOM does that with CSS and touch scrolling for free, and the toolkits' mobile
  stories add build complexity the team does not need.
- *Over a native TUI on the phone (Termius today):* it keeps working, so nothing is lost
  while the web client is built.

A store listing later is a wrapper (Capacitor) around the same page. Saves are the same
action-list text as the TUI's, in local storage, so a run can move between the terminal and the
phone. One check belongs to the first stage: `cargo check -p sts2-core --target
wasm32-unknown-unknown` (the engine uses no threads or files, so it should pass as is).

## 4. Migration plan

Stages 1, 2a and 2b are done (2026-10-27): `sts2-view` owns `text`, `names`, `describe` and
`menu`; the `Screen`/`Input` model and the pure combat builder are in. Stage 2c-1 (2026-10-29) moved
the fight's state machine into `sts2_view::ui::Ui` (`input` returns effects the TUI applies) and
the builder carries the previews, incoming row, last-turn line, target buttons and choice sheet.
Stage 3 (UI3, 2026-10-29) moved the rooms: `sts2_view::rooms::screen` builds events and Neow/
ancients, rewards and the card pick, deck selections, bundles, rest sites and treasure rooms
(boss relics are the rewards and treasure screens, the engine has no separate decision), and
`rooms::end_screen` the run end. A row's `ButtonId` is its number among the enabled choices, a
digit key prints it, and a long list carries every page's rows (`Page::first/len` pick the
visible ones). `Ui::room_input` holds the picks, the card reward opened as its own screen and the
button that asks before it throws something away (`Asking`); `Ui::end_input` turns the end
screen's buttons into `Effect::End`. The TUI draws them from `Screen`; `Paging` and `Visit` moved
to `sts2-view`, and the end layer of `keys.rs` shrank to "other keys do nothing". 
Stage 4 (UI4, 2026-10-29) moved the map, the shop, the `≡` menu and the lists. `rooms::screen`
builds the compact map grid (`MapGrid`: cells, edges, you, boss, kinds) with its next-room rows;
the shop in two levels (its sections with counts, then one open section's items; the open
section has a `Back` button, Esc or a tap, handled by `Ui::room_input`); the run menu and the
deck, relics, potions, piles, log and visited sheets (`Sheet::Menu`, paging kept in `Ui`); and
`Ui::top_bar` the top bar with the potion belt (`TopBar.fight`, `PotionSlot.asks`). Rows the client
carries out itself come back as `Effect::Menu`. Deleted: `ui/overlay.rs` and the shop remains of
`ui/menu.rs` (`hud.rs` and `layout.rs` were gone already). `combat::screen` lost its `Context`
and returns `top: None`; `Ui::screen` lost its `Context` argument; `rooms::end_screen` takes the
`Describer`; `sts2-web` must follow. Still open (stage 5): the inspect overlay and the wide layout.
Stage 5 (UI5, 2026-10-30, partly): `sts2_view::launch` builds the main menu, the character select and the settings page (`Launch` takes `Input`s and asks the client for a `Request`; the saved run, the crash note and the settings file stay in the TUI's `CharacterSelect` wrapper); `sts2_view::details` builds the card, enemy and menu-entry detail sheets. `select.rs`, `inspect.rs` and the old card/power helpers in the TUI are gone. A right click on a card, enemy or choice opens its detail, as a panel beside the column when the terminal has room. Character rows show `N HP` (not `♥ N`) so the chosen character's `♥ hp/max` is the first on screen. 
Stage 5 finished (UI5b, 2026-10-30): rows with details (cards, enemies, and choices with a card, an `info` or detail text) end in `›`; the detail key or a right click opens the sheet from `sts2_view::details`. On a terminal wide enough to leave room beside the column, a right panel shows the details of the focused row (keyboard first); the mouse hovering a row feeds it too but never moves the focus; an open sheet takes the panel's place. `shell.rs` keeps the state, the settings and the run start/continue/autosave; `shell/select.rs` (the `CharacterSelect` wrapper), `shell/input.rs` (key layers and presses) and `shell/mouse.rs` (clicks, drags, wheel) hold the rest. `keys.rs` is the global table the help lists from.
Stage 2c-2 (2026-10-29) finished stage 2: the TUI draws the fight (combat, targeting, choice
sheet) from `Screen` (`ui/combat` reads the model's rows; it keeps the `Plan`/`Big` layouts), hit
regions for cards, End Turn and Done name the rows' `ButtonId`s through `ids`, and snapshots add a
crowded fight (10 cards, 5 enemies, a 6-power enemy). The builder gained the card parts
(`CardFace`), long intents, power visibility, Osty's target, stars, piles, blades, orb slots and
the choice prompt. Each stage is one agent, lands with `cargo xtask gate` green, and leaves the TUI playable
end to end. Stages 2 to 5 keep old screens alive until their replacement draws from the model.

1. **Pure text to a new crate.** Create `sts2-view`; move `text.rs` (markup → `Rich` with
   roles), `names.rs`, `describe.rs`, `menu.rs`. The TUI maps `Role` to styles in `theme.rs`.
   No visible change. Add the wasm32 `cargo check` of `sts2-core` and `sts2-view` to the gate.
2. **The model and the fight.** Add `Screen`, `Block`, `Button`, `Rich`, `Input` and `Ui`
   (moved from `app.rs`); build the combat screen (2.5), targeting and the choice sheet (2.6)
   from the model; the TUI draws combat from `Screen` and its hits are `ButtonId`s. Keys come
   from the rows; delete the digit modes and the combat layers in `keys.rs`. Snapshots at
   44×30, 80×24, 120×35 including a 10-card hand, 5 enemies and a 6-power enemy.
3. **Rooms.** Events and Neow, rewards, card pick, rest, treasure, boss relics, deck
   selections, bundles and run end from the model (2.3, 2.7–2.12, 2.15); delete the old menu
   drawing.
4. **Map, shop, menu, lists.** The compact map grid and its next-room rows (2.4), the shop's
   two levels (2.10), the run menu, deck/relics/piles/log/visited sheets (2.13), paging;
   delete `hud.rs`, `overlay.rs`, `select.rs`, `layout.rs`, `digits.rs`.
5. **Details and width.** The detail sheet and the `›` (2.14) for every object, the wide
   layout with the right panel and hover feeding it, the ASCII glyph set; delete `tooltip.rs`
   and `inspect.rs`. Main menu, character select and settings from the model (2.1, 2.2, 2.16);
   `shell.rs` becomes the event loop only. `keys.rs` is now the global table.
6. **Saves and the web proof.** Move the save format to `sts2-view::save` (files stay in the
   TUI). Add `sts2-web`: wasm-bindgen exports, `serde` on the view types, a plain HTML page
   that renders `Screen` JSON as rows and buttons and plays a full run on a phone browser.
   Not in the workspace gate's test set beyond `cargo check` for wasm32, so simulation and RL
   builds are untouched.

After stage 6 the TUI is the reference renderer and the phone has two ways to play: Termius
and the page. Pixel art, side panels, chords and hover-only information are gone.
