# SpireZero

A Slay the Spire 2 simulator written in Rust, and a bot that plays it.

The simulator follows the real game (v0.111.0) exactly: give it the same seed and the same
moves and you get the same run, down to every card drawn and every point of damage. The bot is
trained entirely inside it.

Not affiliated with Mega Crit. You need your own copy of Slay the Spire 2 for art, audio, the
decompiled reference and the parity oracle; none of it is in this repo.

## Speed

Measured with `cargo xtask bench` on a Ryzen 7 7800X3D (8 cores, 16 threads), release build with
LTO. A fight is an Ironclad starter deck against one of the 22 act 1 encounters, played with
random legal moves until someone dies.

| | 1 core | 16 threads |
|---|---:|---:|
| fights per second | 115,000 | 1,000,000 |
| actions per second | 2.5 M | 22 M |
| time per action | ~400 ns | |
| whole runs per second (random play, ~70 actions each) | 11,400 | 114,000 |

```
cargo xtask bench --mode fights --threads 1
cargo xtask bench --mode fights --threads all --seeds 100000
cargo xtask bench --mode runs --threads 1 --runs 20000
```

A PGO build (`cargo xtask bench --pgo`) adds about 30% on one core. The state is plain data, so
copying a fight for search is cheap and allocation-free once warm
(`crates/sts2-core/tests/it/copy_alloc.rs`). Profiles and the history of each speed-up are in
[`docs/perf.md`](docs/perf.md).

## The bot

Ironclad, ascension 0. Every win is replayed in the engine to check it
([`docs/research/rlb14-playouts.md`](docs/research/rlb14-playouts.md)):

| seeds | wins |
|---|---|
| test, 9,000,000 x 200 | 187 (93.5%) |
| dev, 8,000,000 x 1,000 | 925 (92.5%) |
| train, 1,000,000 x 1,000 | 943 (94.3%) |

It plays fair. It never looks at the draw order, the RNG or how an event will roll; it only uses
what a player can see on screen ([`docs/specs/honesty.md`](docs/specs/honesty.md)). A test fails
the build if any of its choices depend on hidden state (`crates/sts2-diff/tests/it/honest.rs`).

Fights are played by a search over sampled worlds with a combat value net at the leaves. Map,
card reward, shop and rest choices come from a run policy trained with PPO. No human games were
used for any of it.

Most losses are at the act 3 boss: Queen and Test Subject account for 64 of 120 losses on the
train seeds ([`docs/research/rlb16-loss-anatomy.md`](docs/research/rlb16-loss-anatomy.md)). So far
only Ironclad at A0 is trained; [`docs/specs/a10-training.md`](docs/specs/a10-training.md) covers
A10.

To reproduce the test row:

```
cargo build --release -p sts2-diff
target/release/sts2-diff bot --first 9000000 --count 200 \
    --value builtin --value-leaves --value-hp 300 --rollout v2 --value-mix 300 \
    --run-policy builtin --out DIR
```

## Playing

The web client (`crates/sts2-web`) runs entirely in the browser: the engine and the bot are
compiled to wasm, and saves stay in local storage. It works on a phone or a desktop, and the bot
can suggest moves or replay your seed.

```
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
cargo xtask art-import          # art and text from your copy of the game
crates/sts2-web/build.sh
crates/sts2-web/serve.sh        # or any static file server on crates/sts2-web/www
```

There is also a terminal client: `cargo run -p sts2-tui --release`.
More in [`crates/sts2-web/README.md`](crates/sts2-web/README.md) and
[`crates/sts2-tui/README.md`](crates/sts2-tui/README.md).

## How parity is checked

`referee/` runs the real game headless. `sts2-diff` feeds the same moves to both and compares the
full state after every action: combat seeds, whole runs, events, ancients and maps, plus a set of
recorded runs kept as fixtures. On the run loadouts, 497 of 558 cards, all 64 potions and 231 of
242 powers have been exercised. Details in [`docs/regression.md`](docs/regression.md).

Behaviour comes from the game's decompiled code, which you generate locally with
`tools/decompile.sh` into `ref/` (git-ignored). Citations like `Foo.cs:123` in the code point
there, and [`docs/semantics/`](docs/semantics/) collects what has been learned from it.

## What needs the game

Without the game you can build and test the whole workspace (tests that need game files skip),
run the engine, the RL environments and the benchmarks, use the TUI (it shows ids instead of
names), and build the web client.

With your copy (Steam, v0.111.0, found at the default install path, or set `STS2_GAME`):

| for | run |
|---|---|
| art and text in the TUI (`--features embedded`) and the web client | `cargo xtask art-import` |
| sound in the web client | `crates/sts2-web/tools/audio_import.py` |
| the decompiled reference in `ref/` | `tools/decompile.sh` (needs .NET and ilspycmd) |
| the referee and the differential tests | `referee/setup.sh` (also clones [sts2-cli](https://github.com/morefreeze/sts2-cli)) |

Imported data goes to `~/.cache/spirezero-data` (`$SPIREZERO_DATA`).

## Crates

| crate | |
|---|---|
| `sts2-core` | the engine: combat, the run, every card, relic, potion and monster |
| `sts2-macros` | proc macros for hook dispatch |
| `sts2-diff` | differential tests against the real game, and the bot |
| `sts2-view` | screen models shared by both clients |
| `sts2-tui` | terminal client |
| `sts2-web`, `sts2-web-bot` | browser client, and the bot in a Web Worker |
| `sts2-rl`, `sts2-ffi` | RL environments and their C ABI for Python |
| `sts2-bench`, `sts2-kernel` | benchmarks and an experimental fixed-size fight kernel |
| `referee/` | the real game, headless |
| `xtask/` | `cargo xtask gate` (fmt, clippy, tests), `bench`, `art-import`, `coverage` |

Docs start at [`docs/README.md`](docs/README.md); [`docs/CHANGELOG.md`](docs/CHANGELOG.md) lists
changes, newest first.

## Contributing

Port behaviour from the decompiled code; don't guess it, and don't copy it. Fixtures and golden
data are read-only, so if a test fails, fix the engine. Game logic uses no floats (C# `decimal`
is emulated exactly).

## License

MIT, see `LICENSE`. `NOTICE` credits [spire-mentor](https://github.com/tfriedel/spire-mentor)
for adapted code and [sts2-cli](https://github.com/morefreeze/sts2-cli), which the referee builds
on.
