# SpireZero

An AI that plays Slay the Spire 2, running on an exact Rust port of the game's rules (v0.111.0).

Not affiliated with Mega Crit. You need your own copy of Slay the Spire 2 for art, audio, the
decompiled reference and the parity oracle; none of it is in this repo.

## The bot

It plays honestly: it never sees draw order, RNG streams or unrolled event outcomes, only what a
player sees ([`docs/specs/honesty.md`](docs/specs/honesty.md)). A leak gate fails the build if its
actions depend on hidden state (`crates/sts2-diff/tests/it/honest.rs`).

It combines a search over blindfolded worlds, a learned run policy (PPO) for map, reward, shop and
rest choices, a combat value net, and a learned whole-fight playout policy in boss fights. All are
trained in the simulator; no human games are used.

Ironclad, ascension 0, every win replayed in the engine
([`docs/research/rlb14-playouts.md`](docs/research/rlb14-playouts.md)):

| Seeds | Wins |
|---|---|
| test, 9,000,000 x 200 | 187 (93.5%) |
| dev, 8,000,000 x 1,000 | 925 (92.5%) |
| train, 1,000,000 x 1,000 | 943 (94.3%) |

```
cargo build --release -p sts2-diff
V="--value builtin --value-leaves --value-hp 300"
target/release/sts2-diff bot --first 9000000 --count 200 $V --rollout v2 --value-mix 300 \
    --run-policy builtin --out DIR
```

For dev use `--first 8000000 --count 1000`. Most losses come at the act-3 boss: Queen and Test
Subject end 64 of 120 losses on train seeds
([`docs/research/rlb16-loss-anatomy.md`](docs/research/rlb16-loss-anatomy.md)). Only Ironclad at
ascension 0 is trained so far; A10 work is in [`docs/specs/a10-training.md`](docs/specs/a10-training.md). Approaches that did not help are
in [`docs/research/`](docs/research/); `learn/rl/index.html` is a course on the RL setup.

## Play it

**Web UI** (`crates/sts2-web`). You play a run in the browser; the bot can give advice or replay your seed. The layout is built for touch on a phone and works on
desktop. Saves stay in the browser.

```
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
cargo xtask art-import          # art and text from your own game copy
crates/sts2-web/build.sh        # writes crates/sts2-web/www/pkg
crates/sts2-web/serve.sh        # or serve crates/sts2-web/www with any static server
```

Details: [`crates/sts2-web/README.md`](crates/sts2-web/README.md).

**Terminal** (ratatui): `cargo run -p sts2-tui --release`. Options:
[`crates/sts2-tui/README.md`](crates/sts2-tui/README.md).

Both clients render the screens built by `sts2-view`
([`docs/specs/clients.md`](docs/specs/clients.md)).

## The engine

`sts2-core` is a port, not a clone: the same seed and the same actions give the same result as the
real game. Parity is measured, not assumed. `referee/` runs the real game headless, and `sts2-diff`
plays the engine and the referee through the same actions and compares every step: combat seeds,
whole runs, events, ancients and maps, plus pinned run fixtures. The verification stack is in
[`docs/regression.md`](docs/regression.md). Behaviour coverage of the run loadouts: 497 of 558
cards played, 64 of 64 potions used, 231 of 242 powers changed.

## Bring your own game

Works without the game: `cargo build` and `cargo test` of the workspace (tests that need game
files skip), the engine, the RL environments, benchmarks, the TUI (ids instead of names) and the
web UI build.

Needs your copy (Steam, v0.111.0, at its default install path; `STS2_GAME` or `--game-dir` points
elsewhere):

| What | How |
|---|---|
| Game text and art for the TUI (`--features embedded`) and web UI | `cargo xtask art-import` |
| Sound for the web UI | `crates/sts2-web/tools/audio_import.py` |
| The decompiled reference `ref/` | `tools/decompile.sh` (.NET and ilspycmd) |
| The referee and differential tests (`tools/regress*.sh`, `tools/final_*.sh`) | `referee/setup.sh`, which also clones [sts2-cli](https://github.com/morefreeze/sts2-cli) |

Imported data goes to `~/.cache/spirezero-data` (`$SPIREZERO_DATA`). The old `spirebot` names of
these dirs and variables are still read.

## Layout

| Crate | What it is |
|---|---|
| `sts2-core` | The engine: combat, run layer, content (one file per entity). |
| `sts2-macros` | Proc macros for the engine's hook dispatch. |
| `sts2-diff` | The differential harness, and the honest bot (`bot`, `fights`, training tools). |
| `sts2-view` | The presentation model shared by the clients: screens, text, menus. |
| `sts2-tui` | The terminal client. |
| `sts2-web`, `sts2-web-bot` | The browser client (wasm plus page) and the bot in a Web Worker. |
| `sts2-rl`, `sts2-ffi` | The RL environment and its C ABI for Python. |
| `sts2-bench`, `sts2-kernel` | Benchmarks and an experimental fixed-capacity fight kernel. |
| `referee/` | The real game, headless, as the test oracle. |
| `xtask/` | Automation: `cargo xtask gate` (fmt, clippy, tests), `art-import`, `coverage`. |

Docs start at [`docs/README.md`](docs/README.md); changes, newest first, in
[`docs/CHANGELOG.md`](docs/CHANGELOG.md).

## Truth and rules

The decompiled game is the authority for behaviour. `tools/decompile.sh` makes it locally under
`ref/` (git-ignored); citations like `Foo.cs:N` in code and docs point into it.
[`docs/semantics/`](docs/semantics/) is the ledger of facts learned from it.

Port from the decompilation, never guess. Golden data and fixtures are read-only: fix the code,
not the test. No floats in game logic (C# `decimal` is emulated exactly). State is plain data: ids,
no `Rc`/`RefCell`/`dyn`. Never copy decompiled code into the engine.

## Licence

MIT, see `LICENSE` and `NOTICE`. Parts adapt [spire-mentor](https://github.com/tfriedel/spire-mentor)
and build on [sts2-cli](https://github.com/morefreeze/sts2-cli).
