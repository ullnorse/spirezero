# Disk use of many worktrees (DK1)

Up to 5 workers each have a git worktree, and each builds, tests and regresses in its own `target/`. On the old machine the
`target/` directories together reached ~1 TB. This page has the measurements and the recommendation. Tool: `tools/disk.sh`.

## One worktree, before and after

One checkout after `cargo xtask gate`, `tools/regress.sh 5` and `cargo xtask bench` (no `package`), 2026-10-24:

| | before (2bb0394's start) | after the profile fix and the diff cleanup (2bb0394) | after DK1 |
|---|---|---|---|
| target/regress/work (traces of passing seeds) | 12 GB | ~0.3 GB (actions + hits only) | same |
| target/debug, clean `cargo test --workspace --no-run` | 9.1 GB | 3.8 GB | **2.9 GB** |
| target/debug after a few edits | | 3.8 GB+ | grows by ~0.45 GB per edit-build until trimmed (`tools/disk.sh --gc`) |
| test binaries (>25 MB each) | 25 | 25 | 13 (8 of 80-100 MB) |
| target/regress (release profile build) | 1.1 GB | 1.1 GB | 1.1 GB |
| target/fast | 129 MB | 129 MB | 129 MB |
| referee trace store (shared by all worktrees) | 5.7 GB, 34.8k plain files | same | **140 MB** (gzip -6; 196 MB on disk with 4 KB blocks) |

Test counts: `cargo test --workspace` ran 618 passed, 0 failed, 3 ignored in 27 test executables before and the same 618 / 0 / 3 in 13
after (integration binaries: sts2-core 8 files -> 1, sts2-diff 2 -> 1, sts2-tui 7 -> 1; sts2-kernel's single file is unchanged).

What is left in a debug checkout (2.9 GB clean): incremental 1.4 GB, deps 1.5 GB (workspace crates and their test binaries ~1 GB, third
party ~0.5 GB), build scripts 26 MB. Each of the 8 big binaries is 80-100 MB, of which 70% is the symbol table and line tables (a stripped copy is
30 MB). The debug info was kept as `line-tables-only` on purpose (panic backtraces); `[profile.dev] debug = false` would roughly halve the
deps again, and is the next step if it is needed.

## Test binaries

Every `crates/<crate>/tests/*.rs` file is its own binary that links its own copy of the crate under test. They are now one binary
per crate, `tests/it/main.rs` with one module per old file: `cargo test -p sts2-core --test it golden_rng` runs the old
`golden_rng` file (the module path is part of every test's name now: `golden_rng::...`). Costs of the move: the tests of
a crate share one process, so a panic hook set by one test is seen by the others (`fuzz_combats` replaces the hook while it runs and
`crash` installs one; neither matters unless another test fails in that window), and a change to any test file relinks the one
binary instead of one of several. New integration tests go in as a module of `tests/it/main.rs`, never as a new file in `tests/`.

## Trace store

`referee/trace_cache.py` writes `<key>.jsonl.gz` (gzip level 6, mtime 0 so equal traces are equal bytes) and still reads plain
`<key>.jsonl` (first, so converting can delete it after writing the `.gz`). Level 1 gives 5.2% of the size, level 6 2.4%, level 9
2.2%, zstd (Python 3.14 only) 2.1%: gzip 6 is what is portable. Measured on 300 traces of ~175 KB: compress 0.5 ms, decompress 0.12 ms,
against a 0.07 ms plain read. A full `tools/regress.sh 5` (6,175 runs, 30,875 traces from the store, 0 played) took 120 s from the plain store and 124 s
from the compressed copy, i.e. no difference. `python3 referee/trace_cache.py compress` converts an existing store in place (6.0 GB -> 140 MB) and is safe while other workers run, **but only once every worker runs the new `trace_cache.py`**: older code reads
only plain files and would play the referee for every trace. So run it after DK1 is merged and the other worktrees have it.

## `tools/disk.sh`

No arguments: a table of `target/` per checkout (from `git worktree list`; debug, regress, fast, other), `referee/builds` of the main
checkout, the `~/.cache/spirezero-*` caches and the free space. `--gc` (`-n` first to see what goes) is safe while workers run:
it prunes worktrees whose directory is gone, deletes `target/` of a worktree whose branch is merged into main unless the checkout looks
in use (a process with its cwd or binary there, a cargo build holding a profile lock, `target/` written in the last 2 hours: a task
branch that was just created from main also counts as merged), trims every other checkout's artifacts, incremental sessions and diff/regress work
directories untouched for `--days` (3), while holding cargo's own `.cargo-lock` of the profile (a build starting meanwhile waits; one in progress makes the
tool skip the profile), and drops traces unread for 30 days. It never touches source, `referee/builds`, or anything outside `target/` and the trace store.
Tested on a scratch repo with a merged, a live and a vanished worktree.

## Alternatives measured or considered

- **Shared `build.build-dir`** (`CARGO_BUILD_BUILD_DIR`, final artifacts stay in each `target/`): measured with two copies of the
  tree. The second worktree compiled only the workspace crates (15 s instead of ~70 s) and reused third-party builds, but the shared
  directory grew from 2.9 GB to 5.3 GB: workspace crates are keyed by their path, so they are not shared, and third party is only ~0.5 GB
  of ~3 GB. It also makes concurrent builds in different worktrees queue on one `.cargo-lock` per profile, which is the opposite of 5 parallel workers.
  Saves ~0.5 GB per worktree at the price of serialising builds: not worth it.
- **sccache**: not installed and not measured. It needs non-incremental builds (12 s instead of 4.7 s per edit in the same tree), caches by command line
  including absolute paths (a worktree per task means no hits for workspace crates unless paths are remapped), and the only large reusable part
  is the 0.5 GB of third-party code. A cache that grows without bound would replace a `target/` that grows.
- **Non-incremental dev builds** (`CARGO_INCREMENTAL=0`): measured. `target/debug` after three edit-builds 1.6 GB instead of 3.8 GB, an edit
  of one card rebuilds and relinks everything in 12 s instead of 4.7 s. Worth it for a worker that only runs the gate once and finishes;
  not for a long iteration. Not made the default.
- **Bazel (rules_rust)**: not installed, not measured; from the docs. A disk cache is content-addressed and shared by all workspaces, so identical
  sources in several worktrees are built once, which does attack the duplication. Against it: `crate_universe` must mirror Cargo.lock and every
  `build.rs` (sts2-core generates its registries in one), `cargo xtask`, the cargo aliases and `[profile.*]` settings in the root `Cargo.toml` would need
  a second description, incremental compilation is not available (a content change in `sts2-core` rebuilds the whole crate in each action), each
  workspace has its own output base of several GB anyway, and the toolchain, `rust-version` and clippy settings would be configured twice.
  A hard blocker for cargo was not found, so we stay on cargo.

## Recommendation

Stay on cargo. Keep one test binary per crate, `[profile.dev]` as it is, the compressed trace store, and run `tools/disk.sh --gc`
routinely (see WORKFLOW, "Disk"). With the profile fix, the single test binaries and the gc, one worktree stays near 3-6 GB (debug 3 GB, regress 1.1 GB, fast 0.1 GB), i.e. 15-30 GB
for 5 workers instead of the old 1 TB.
