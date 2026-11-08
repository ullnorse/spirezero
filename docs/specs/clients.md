# Clients: TUI, iOS and RL

One engine, three consumers. The engine decides; a client draws and sends input.

```
                    sts2-core (rules · run · content)
                   /               |                 \
          sts2-rl                 sts2-view          sts2-diff
       obs · masks · envs     Screen · Input · Ui     referee harness
              |                    |
          sts2-ffi          +------+--------+
       batch C ABI          |               |
              |          sts2-tui      sts2-mobile
          python/       ratatui         C ABI (JSON)
        trainer                            |
                                        apple/ (SwiftUI)
```

## Dependency rules

| Crate | May use | Must not use |
|---|---|---|
| `sts2-core` | nothing | UI, RL, serde for presentation, files, threads |
| `sts2-view` | `sts2-core` | ratatui, crossterm, filesystem, threads, RL |
| `sts2-tui` | `sts2-view`, terminal libs, `sts2-diff` (only the honest bot behind the `w` hint key, on its own thread) | core decisions, RL |
| `sts2-rl` | `sts2-core` | `sts2-view`, UI |
| `sts2-ffi` | `sts2-rl` | view/UI |
| `sts2-mobile` | `sts2-view` | `sts2-rl` |
| `sts2-diff` | core | shipping paths |

Two consequences are intentional:

- **RL never sees `Screen`.** Training reads core state through observations and writes actions
  through masks; the presentation model is not on the hot path.
- **Clients never see `Run` for decisions.** Every label, number, preview, legal action and
  confirmation comes from the engine through the view model.

## `sts2-view`

Stage 1 (done 2026-10-27): `text` (markup → `Role`-tagged `Rich`, no terminal types),
`names`, `describe` and `menu`. The TUI's `text` module is now only the JSON loader and the
`Rich → ratatui` adapter; `names`/`describe`/`menu` are re-exports.

Stage 2a (done 2026-10-27): the `model` types (`Screen`, `Block`, `Button`, `Input`, sheets)
with JSON round-trip tests, so the iOS ABI has a target. Stage 2b: the pure combat builder
(`combat::screen`) now reads a `Combat` into the model; the TUI renderer and `Ui` input are the
next step. `Ui::input(Input)` is the only way in; `Ui::screen()` is a pure function
of `(Run, UiState)`. The TUI draws `Screen` and maps key/mouse events to `Input`; the iOS app
does the same in SwiftUI.

Stage 5 (done 2026-10-30): `launch` builds the main menu, character select and settings;
`details` builds the detail sheets. A row's `›` (`Button::info` or `detail`) means a detail
sheet exists; `Input::Info(id)` opens it. A client with room shows the focused row's details
in a side panel instead of a sheet; hover may feed it but never moves the focus. The TUI's
`shell` is split into `select`, `input` and `mouse`; `keys` is the global table.

`sts2-view` must build for `wasm32-unknown-unknown` (the gate checks it) and for the iOS
targets; that is the test that it has no terminal or filesystem dependency.

## Session and saves

Seed + character + ascension + action list + state digest, as text. The pure format lives with
the view model (`sts2-view::save` when stage 2 extracts it); file paths, crash notes and
`SPIREBOT_HOME` stay in the renderer. A save moves between the terminal, the phone and a test
unchanged; playback checks the digest and warns on a different build.

## iOS

- Rust: `sts2-core` + `sts2-view` + `sts2-mobile` as a `staticlib` for `aarch64-apple-ios` and
  the simulator.
- ABI: `new(seed, character, ascension)`, `restore(save)`, `screen_json()`,
  `input_json(json)`, `save_json()`, `free()`. JSON first: `Screen`/`Input` are serializable,
  Swift decodes `Codable` structs, and there is no generated-binding drift. UniFFI can replace
  it later if the surface grows.
- Every call is wrapped in `catch_unwind`, like `sts2-ffi`; no engine pointers cross the ABI.
- SwiftUI: one observable holds the current `Screen`; a tap/key becomes `Input`. Main menu,
  continue, settings and the save file live in the app.
- Assets and localization are bundled by the app; `sts2-view` only sees providers/tables.
- Build and signing need macOS/Xcode (the xcode project comes later). Personal use only: never
  distribute a binary that carries the game's assets or text.

## RL

`sts2-rl` and `sts2-ffi` already fit this split and do not change for the client work. The
contract, frozen seeds, observation/action space and baselines are in `a10-training.md`; the
long-term objective is A10, but the current target is A0 Ironclad first (user, 2026-10-27).

- Train on `RunEnvs` at A0 first, then A10, with fixed seed sources; combat-only for throughput
  and full-run for the objective.
- Evaluate on held-out seeds and the history replay outside the training loop; the referee is
  an oracle, not a training dependency.
- A trained policy is just another action source; it does not enter `sts2-view` unless a
  bot-played UI is built later.

## Sequencing

1. `sts2-view` stage 1 — **done**.
2. Stages 2 and 3 are **done** (2026-10-29): the model, the pure combat builder, `Ui` and the TUI
   drawing the fight from `Screen`; then `sts2_view::rooms` (events and Neow/ancients, rewards,
   card pick, deck selections, bundles, rest, treasure, run end) with `Ui::room_input` and
   `Ui::end_input`, the TUI drawing those from `Screen`. Stage 4 is **done** (2026-10-29): the map
   (`MapGrid`), the shop's sections, the `≡` run menu and the lists (`Sheet::Menu`) and the top
   bar (`TopBar.fight`, `PotionSlot.asks`, `Ui::top_bar`) are built in `sts2-view`; menu rows a
   client carries out come back as `Effect::Menu`. Signature changes for other clients
   (`sts2-web`): `combat::screen` has no `Context` and returns `top: None` (use `Ui::top_bar`),
   `Ui::screen` lost its `Context` argument, `rooms::end_screen` takes a `Describer`.
3. Extract the pure save format; cross-client playback test.
4. `sts2-mobile` C ABI; minimal SwiftUI app when Xcode is available.
5. TUI and iOS iterate on the same model; RL stays parallel and independent.

Do not start iOS before stage 2, and do not put `sts2-view` in the RL path.
