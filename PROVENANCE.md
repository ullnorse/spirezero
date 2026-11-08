# Provenance

SpireZero reimplements the rules of **Slay the Spire 2** ((c) Mega Crit). It is not affiliated with
Mega Crit. This file says what in the repository is original work and what from the game is left
out. The public repository is an export of a private development repository; the export boundary is
enforced mechanically by an exporter with path excludes and content gates.

## Not in this repository

| What | Why | How you get it |
|---|---|---|
| The decompiled game (`ref/`) | Verbatim game code | `tools/decompile.sh` from your own copy |
| Game art, audio and text (`crates/sts2-web/www/{art,audio,text}`, the TUI's embedded text) | Game assets | `cargo xtask art-import`, `crates/sts2-web/tools/audio_import.py` |
| sts2-cli and the game's localization (`referee/vendor/`) | Third-party code and game text | `referee/setup.sh` |
| The game's assemblies, `.pck` files | Game binaries | Your install |
| Act-jump loadouts drawn from the community run dump | Player data with no licence | Not distributed |

All of these paths are git-ignored. Code and docs cite the decompilation by path (`ref/decomp/…`,
`Foo.cs:N`) and by namespace (`MegaCrit.…`); those are references into your local copy, not
content. Comments paraphrase single game API calls; no block of decompiled code is quoted (the
exporter fails on a `csharp` fence or on 3+ consecutive lines shared with the decompilation).

## Original work (MIT, `LICENSE`)

- The Rust engine, harness, clients, RL environments, benchmarks and xtask (`crates/`, `xtask/`).
- The referee's C# sources (`referee/tracer`, `stubs`, `patcher`, `nullguard`, `stubcheck`) and
  `tools/golden`: they call the game's public API and copy none of its code. `referee/stubs/Mathf.cs`
  re-implements Godot's `Mathf` API (see `NOTICE`).
- Tools, docs, the semantics ledger, `docs/inventory/` (generated lists of ids and paths), and
  `learn/`.
- Trained nets in `crates/sts2-diff/data/`: trained on the engine's own self-play.
- Recorded traces (`crates/sts2-diff/run_fixtures/`, `referee/examples/`): seeds, actions and the
  numbers of each step, recorded from the game as the parity oracle. No game text or assets.

## Third-party code

`NOTICE` lists it: code adapted from spire-mentor (MIT), and the projects the referee builds on.
