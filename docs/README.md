# Docs

Start here, in this order:

| File | What it is |
|---|---|
| [`CHANGELOG.md`](CHANGELOG.md) | What changed, newest first, without reading every commit. |

## Truth sources

- `../ref/` is the decompiled game (made locally by `tools/decompile.sh` from your own copy; read-only, start at `ref/INDEX.tsv`). It is the
  authority for game behaviour; never guess.
- `semantics/` is the write-only ledger of facts learned from the decomp, split by topic.
  `semantics.md` is the index. Append a fact with its decomp reference; never rewrite one.
- `inventory/` is the generated entity inventory (20 character/act pairs). Read it through
  `cargo xtask inventory <id>` (`--deps` for dependencies), not as a whole file.
- `specs/` holds designs and how-tos. The active ones are below; a superseded one moves to
  `history/`.
- `reviews/` holds dated reviews (core, perf, TUI, spire-mentor) with their flamegraphs.
- `history/` holds dated records (sign-offs, reports, recovery) and superseded specs. Nothing
  there describes today's state.

## Active specs

- `specs/ui-reimagined.md`: the TUI direction (`sts2-view` + the phone-shaped screens). Supersedes
  `tui-clean.md` and `tui-redesign.md`, both in `history/`.
- `specs/clients.md`: the split between the TUI, the iOS app and RL, with the dependency rules.
- `specs/honesty.md`: what the bot may see; the clairvoyance rules and the tests before a result.
- `specs/a10-training.md`: the RL objective, frozen seed sets, observation/action contract, reward
  and the A10 baseline.
- `specs/content-api.md`: how to add a card, relic, potion, monster, event or ancient.
- `specs/hooks.md`: the hook table (when each hook fires, call site, status).
- `specs/run-layer.md`: the run layer (map, rooms, rewards, shops, events, ancients).
- `specs/test-mode.md`: the seven TestMode branches the referee runs with.
- `specs/rl-2x-plan.md` and `specs/stage4-design.md`: the parked RL/core-layout plan.
- Per-character and per-act porting references: `characters.md`, `necrobinder-act1.md`,
  `ironclad.md`, `silent.md`, `defect.md`, `regent.md`, `orbs.md`, `underdocks.md`,
  `combat-loop.md`, `rng-decimal.md`, `content-gaps.md`.
- Performance: `perf.md` (measurements, append-only) and `referee-perf.md` (why the referee is
  fast enough to be the oracle).

## Commands

| Command | What it checks |
|---|---|
| `cargo xtask gate` | fmt, clippy `-D warnings`, workspace tests, the wasm build of `sts2-core`/`sts2-view`. The merge gate. |
| `cargo xtask status` | Branch, worktrees, unmerged branches, referee build, brief counts. |
| `cargo xtask merge-check <branch> <hash>` | The branch contains the worker's reported head. |
| `cargo xtask docs-check` | Relative links in the active docs resolve. |
| `cargo run -p sts2-diff -- oracle-gaps` | The behaviours the referee cannot check. |
| `cargo xtask coverage` | Every inventory entity is ported (exit 1 otherwise). |
| `tools/regress.sh 5` | Every combat encounter at A0/A10 against the referee (6,175 runs). |
| `tools/regress_runs.sh 8` | The whole-run loadouts (499 runs). |
| `tools/final_matrix.sh --acts all 200` | The full sign-off matrix (about an hour). |
| `tools/hits_sweep.sh` | Every ported entity is reached by a generated run; `tools/hits.sh --effects` also reports entities whose effect never ran. |
| `tools/replay_history.py` | Real run histories (maps, events, rewards) against the engine. |
| `tools/disk.sh [--gc]` | Checkout/cache sizes; `--gc` is safe while workers run. |

Details of the regression stack, cache keys and matrix cells: [`regression.md`](regression.md).

## House rules

- Facts go to `semantics/<topic>.md` with a decomp reference; designs go to `specs/`; dated
  results go to `history/`. The root of `docs/` stays short.
- `decisions.md` is append-only. A reversal is a new entry, not an edit.
- Never edit a fixture, golden file or benchmark to make a change pass. A fixture changes only
  for a real parity fix, explained in the commit.
- A new fact that contradicts an old one is appended with the date and the reason; the old
  fact stays for the record.
