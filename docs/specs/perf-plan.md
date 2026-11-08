# Speed plan: catch spire-mentor (task PF1)

The goal is a SpireZero that is faster than spire-mentor (`../spire-mentor/sts2-rs`, MIT,
by Thomas Friedel). This page gives measured baselines for both engines on this machine, where the time
goes, and a ranked list of changes. Each change says how it is verified without moving parity.

## Baselines (2026-10-23, this box)

Machine: AMD Ryzen 7 7800X3D (8 cores / 16 threads, 96 MiB L3), WSL2 Linux 6.18, shared with other
workers. Load average was 5-6 for the numbers below and 12-30 during other runs. Treat ±8% as noise.
One-core runs are pinned with `taskset -c 9`.

Builds:
- spire-mentor `8e0e917`, `cargo build --release -p sts2-bench --example site_random_bench`
  (Rust 1.96; fat LTO, `codegen-units = 1`).
- SpireZero `a921b5d` plus this task's bench, `cargo xtask bench` (plain `--release`, no LTO).
- Both use glibc malloc.

**Workload W1** is spire-mentor's site benchmark, and `crates/sts2-bench` re-creates it:
- Ironclad, A0, the starter deck, 80/80 HP, Burning Blood.
- The 22 Overgrowth encounters round-robin.
- Each seed's start state is built untimed. Every fight clones it (timed) and plays uniformly random legal
  actions to the end.
- 2,200 seeds × 50 fights.

The two engines play the same distribution of fights, not the same fights: the encounter order and policy
RNG differ, and SpireZero plays v0.111.0 where spire-mentor's W1 plays v0.107.1. The aggregates still line
up:

| | wins | actions per fight |
|---|---:|---:|
| spire-mentor | 32.41% | 21.98 |
| SpireZero | 32.61% | 21.93 |

spire-mentor's identity line matches its published run exactly (`digest c78f3428460d718b`, 35,646 wins,
2,417,920 decisions). SpireZero's identity is `digest ad55be60f5bd8a71`, 35,868 wins, 2,412,377 actions.

| W1 | spire-mentor | SpireZero | ratio |
|---|---:|---:|---:|
| 1 core, fights/s (median of 3, ABAB) | 73,100 | 6,340 | 11.5× |
| 1 core, actions/s | 1,608,000 | 139,000 | 11.6× |
| 1 core, ns/action | 622 | 7,200 | |
| 16 threads, fights/s (sum of per-worker rates) | 660,000 | 59,300 | 11.1× |
| 16 threads, actions/s | 14.5 M | 1.30 M | |

Notes on the table:
- spire-mentor ran 16 pinned processes. SpireZero ran 16 threads; its wall-clock rate was 1.0 M actions/s
  because each thread did little work and the threads finish unevenly.
- Scaling over one core is about 9× on both engines, as expected for 8 SMT cores.
- spire-mentor publishes 59,400 fights/s per core on an EPYC 7443P. This CPU runs it 1.23× faster.

**Whole runs** (SpireZero only; spire-mentor has no equivalent bench):
- Necrobinder A0, random Act 1, uniformly random policy.
- Random play dies on floor 1-2, at about 72 actions per run.
- 1 core: 29 runs/s, 2,100 actions/s (467 µs/action).
- 16 threads: 309 runs/s (load 12).
- 200-run digest: `4a353e75d1d6e2b9`.

Reproduce:

```sh
cargo xtask bench                                   # fights 2200x50, then 200 runs, 1 thread
cargo xtask bench --mode fights --threads all       # or --threads N; --seeds/--reps/--runs/--seed-start
taskset -c 9 target/release/sts2-bench --mode fights --seeds 2200 --reps 50
# spire-mentor (from sts2-rs/):
taskset -c 9 target/release/examples/site_random_bench --seeds 2200 --reps 50
```

Pin with `taskset` when comparing builds: an unpinned `cargo xtask bench` at the same load measured 4.2k
fights/s against 6.3k pinned. The digest, wins and actions do not depend on `--threads`. **Any optimization must leave them identical**
(for fights at 2200×50: `ad55be60f5bd8a71`). This is the cheapest behaviour check, and it runs before
`tools/regress.sh`.

## Where SpireZero's time goes

Profiled with `perf record -g`. The WSL2 build of `perf` at `/usr/lib/linux-tools/*/bin/perf` works at
`perf_event_paranoid=2` but cannot symbolise, so `tools/perf_symbolize.py` names the frames. Use a
frame-pointer build: `RUSTFLAGS="-C force-frame-pointers=yes" CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`
with its own `CARGO_TARGET_DIR`.

**Fights** (3.9k samples, inclusive %):

| Share | What | Where |
|---:|---|---|
| 56% | `Combat::legal_actions` | `combat/mod.rs:415` |
| 62% | `check_play` / `can_play` (part of `legal_actions`, plus the re-check in `apply`) | `mod.rs:690`, `mod.rs:720` |
| 60% | `CombatState::listener_entries`, mostly from `can_play` | `combat/hooks.rs:1221` |
| 18% | `has_keyword` / `keywords`, from `can_play` | |
| 14% | `star_cost`, from `can_play` | |
| 13% | `energy_cost`, from `can_play` | |
| 26% | `Vec<Entry>` growth (`grow_one`, `finish_grow`) | |
| 28% | libc malloc / realloc / free, self | |
| 41% | `Combat::apply` | |
| 21% | `end_player_turn` (enemy turn, next `start_turn`) | |
| 8% | `after_card_changed_piles` dispatch | |
| 9% | `bind` + `Bound::hooks` + `CardKind::model` | |
| < 1% each | clone of the start state (0.3), RNG, decimal, history push (1.2), shuffle (2.0) | |

`can_play` builds a fresh listener list (every power, relic, potion, and every card in every pile) **four
times per (card, target) candidate**: once each for keywords, energy, stars and `should_play`. That is
about 20 rebuilds per `legal_actions` with a 5-card hand. `docs/perf.md` measured three per candidate; the
keyword query has been added since. The engine's own cached list (`Engine::hooked`, `engine.rs:339`) is
rebuilt on every `apply`, because `Engine::new` starts with an empty list, and after every pile or power
change.

**Whole runs** (3.8k samples): **96% is `Run::new` → `ActMap::generate` → `prune_and_repair`**.
- `find_matching_segments` (79%) enumerates every root-to-boss path.
- It `format!`s a `String` key per sub-segment in `segment_key` (58%, mostly `core::fmt`) into a
  `BTreeMap<String, _>` (`run/map.rs:845-912`).
- It does all this from scratch after every prune.
- About 32 ms per run, against about 0.4 ms for the 72 actions played.

## How spire-mentor is fast

Read-only survey. Paths are under `spire-mentor/sts2-rs/crates/`.

**No listener vector.**
- Power listeners are a borrowed iterator over each creature's power uids (`sts2-core/src/hooks.rs:9706`).
  Relics are a separate iterator (`hooks.rs:9764`).
- Before walking, a 512-bit "power kinds present" set (`sts2-core/src/lib.rs:925-951`) and a per-hook
  liveness `u64` (`hooks.rs:510-640`) let a dead hook cost one AND.
- Relic handlers are frozen function-pointer tables with a per-combat `RelicPlan` bitmask
  (`relic_effects/channels.rs:276-330`, `:1310-1360`).
- Debug builds re-run every skipped body on a clone and assert that nothing changed (`hooks.rs:800-830`).

**Cards are not listeners.**
- Cost is the base cost plus a global walk of the modifier powers, skipped when none is present
  (`hooks.rs:13030-13120`).
- The few card-driven sweeps are gated by "this card id exists" bits (`sts2-core/src/cards.rs:1481-1530`).

**`legal_actions`** (`lib.rs:12658`):
- Returns a `SmallVec<[Action; 16]>`.
- Makes one fused pass over the hand.
- Computes the cost-modifier mask once per call.

**State layout.**
- Dense `NonZeroU32` uids with slot lookup (`lib.rs:806-875`).
- Piles are `SmallVec`s of uids (`lib.rs:1699`).
- `Card` is `Copy` (224 B), with `InlineKv` vars (`inline_kv.rs`).
- The whole state is about 7 KB.

**Other.**
- No per-action snapshot.
- Fat LTO.
- PGO only for training (plain 2.46× → PGO 3.16× over its old engine, i.e. about 1.28× from PGO).
- Their "engine-speed work" took W1 from 13,549 to 6,525 instructions per action (2.25×).

**Map pruning** (`sts2-core/src/map_path_pruning.rs:29-62`):
- A segment index is built once and refreshed incrementally after each prune.
- Paths are never materialised; they are rank intervals cleared from a bitset.
- Keys are the C# key's ASCII bytes, in a deterministic hash index sorted ordinally afterwards.
- 40.8 → 1.1 ms per map (36×).
- It is pinned by `sts2-core/tests/map_gen_digest.rs` against digests recorded before the optimisation
  (2,520 seed × config cases).

SpireZero already has some of the pieces: plain-data cloneable state, `HookSet` override bitmasks per model,
a listener cache, and decimal and RNG costs that are negligible. The gap is that it re-derives the listener
set far too often and walks every card for every query.

## Ranked plan

The gains are on W1 one-core throughput unless stated, estimated from the profile shares above. They
compound roughly multiplicatively.

Parity rules for every step:
- The game's listener order (`LISTENER_ORDER`, `combat/state.rs:65`) must not change.
- Nothing nondeterministic.
- No change to what any hook returns.

| # | Change | Expected gain | Parity risk |
|---|---|---|---|
| 1 | One listener list per `legal_actions` | ~1.8-2× | very low |
| 2 | Sparse listener list plus a union `HookSet` | ~1.2-1.3× | very low |
| 3 | Keep the engine's cached list valid | ~1.1-1.15× | low-medium |
| 4 | Map generation without `format!` and from-scratch rescans | 3-4× (step a) or 15-25× (step b) on whole runs | medium |
| 5 | Per-kind constant `HookSet` tables | ~5-10% | low |
| 6 | Allocation diet | ~10-20% | none |
| 7 | Build: fat LTO + `codegen-units = 1`, then PGO | +5%, then +10-25% | none |
| 8 | Presence masks / hook-liveness words | ~1.3-1.5× | medium |
| 9 | State layout | for searches and training | low each |

### 1. One listener list per `legal_actions`

Gain ~1.8-2× (`legal_actions` falls from 56% to about 10%). Parity risk: very low.

- Build the list once per `legal_actions` call, and once per `check_play` inside `apply`.
- Pass `Hooked` to a `can_play_with(h, card)` that uses `h.keywords`, `h.energy_cost`, `h.star_cost` and
  `h.should_play` (`combat/mod.rs:415-449`, `:690-740`; `hooks.rs:1361-1411`, `:1841-2024`).
- `legal_actions` takes `&self`, so nothing changes between candidates. The results are the same by
  construction.
- `docs/perf.md` idea 1, now with a fourth query.

### 2. Sparse listener list plus a union `HookSet`

Gain ~1.2-1.3× after #1. Parity risk: very low.

- In `listener_entries` (`hooks.rs:1221`), skip any listener whose `HookSet` is empty. Strike, Defend and
  Bash override nothing (`#[sts2_macros::hooks]` is on all 1,308 `impl Hooks` blocks).
- Keep the union of all sets so that `Hooked::overriding` and `dispatch` return at once for a hook no
  listener has.
- Filtering preserves relative order by construction.
- The list drops from about 20 entries to about 3 here, so it grows and allocates less.
- Check every reader of `listeners` that does not filter by hook. Add a test that the sparse list equals
  the dense one filtered to non-empty sets, over the sweep's states.

### 3. Keep the engine's cached list valid

Gain ~1.1-1.15×. Parity risk: low-medium.

- Move the cache into `Combat` so it survives across `apply` calls (today `Engine::new` empties it).
- Call `listeners_changed` only when the moved card, its affliction or its enchantment has a non-empty
  `HookSet`. With #2, a hookless card is not in the list at all.
- Covers `docs/perf.md` idea 6 without incremental patching.
- The existing debug oracle (`engine.rs:339-350`, fresh == cached) catches any miss. The `regress` profile
  keeps debug assertions, so `tools/regress.sh` and `cargo xtask sweep` exercise it on every step.

### 4. Map generation without `format!` and from-scratch rescans

This change is for whole runs and does not touch fights. Parity risk: medium for step b, low for step a.

**Step a** (3-4× on map generation):
- Write the key's ASCII bytes into one reused `Vec<u8>` with hand-rolled digit pushes.
- Key a `BTreeMap<Vec<u8>, _>`. Byte order equals the current `String` order, so groups and RNG draws are
  unchanged.
- Store segments as ranges into the path instead of `to_vec`.

**Step b** (15-25× on whole random runs: about 29 → 400-700 runs/s):
- Port spire-mentor's `SegmentIndex`: incremental refresh, rank intervals, deterministic hash index
  (`map_path_pruning.rs`, MIT).
- Credit it in a file header and a root `NOTICE`.

Verify:
- Before starting, record a map digest test: maps plus `map` RNG counter over at least 2,000 seeds × A0/A10
  × Overgrowth/Underdocks, with the current code, as spire-mentor did.
- `tools/diff_map.sh` and `sts2-diff compare-map` against the referee.
- `tools/replay_history.py` (138 runs' maps).
- The runs digest of `cargo xtask bench`.

### 5. Per-kind constant `HookSet` tables

Gain ~5-10%. Parity risk: low.

- Replace `bind(..).hooks()` (`hooks.rs:1274`, `:1139`; a giant `match` plus a `dyn` call per entry) with
  a `[HookSet; N]` table per kind.
- Add a test that the table equals `model().hooks()` for every kind.
- `docs/perf.md` idea 2.

### 6. Allocation diet

Gain ~10-20% after #1-3. Parity risk: none.

- Pre-size the result of `legal_actions`, or return a `SmallVec`.
- Return targets from `card_targets` as an iterator, not a `Vec`.
- Use a stack buffer or `SmallVec` for `dispatch`'s snapshot `Vec<Listener>` (`hooks.rs:2145`).
- Reuse the engine's listener buffer.
- Later, make piles `SmallVec<[CardId; N]>` as spire-mentor does, which also makes clones cheaper.
- A new dependency (`smallvec`) needs the project's say-so.
- jemalloc measured +4% via `LD_PRELOAD`. Revisit an allocator only after this step.

### 7. Build: fat LTO + `codegen-units = 1`, then PGO

Gain +5% measured for LTO, then +10-25% expected for PGO. Parity risk: none for integer logic.

- The f64 RNG and AI-weight code stays IEEE-exact; no `-ffast-math` exists in Rust.
- Add a `[profile.release]` (or a `bench` profile, if release build time matters to the workers).
- PGO: train on `sts2-bench`, following spire-mentor's `scripts/pgo_wheel.sh`. Its identity trace diff
  maps to our digests.
- Gate and digests must stay identical.

### 8. Presence masks / hook-liveness words

Gain ~1.3-1.5× more. Parity risk: medium.

- Keep a per-hook live count (or bit), updated where `listeners_changed` is called. A query with no
  listener then never touches the list.
- Then stop treating hookless cards as listeners at all. This is spire-mentor's design.
- Add debug shadow checks like spire-mentor's `hooks.rs:800-830`: run the full walk and assert the same
  result.

### 9. State layout

Mostly matters for searches and training, which clone per step. Parity risk: low for each step.

- Dense slot ids.
- `Copy` cards with inline vars.
- `SmallVec` piles and powers.
- Clone is 0.3% of W1 today, so measure with a clone-per-step workload (spire-mentor's W2,
  `examples/xsim_toadpoles.rs`) before investing.

### Projections

| After | Estimated W1, 1 core | vs spire-mentor (73k) |
|---|---|---|
| #1-3 | 2.5-3× (about 16-19k fights/s) | |
| #1-7 | 4-5× (about 25-32k fights/s) | about 2.5× slower |
| #8-9 | | needed to close the gap |

Beating spire-mentor needs #8-9 and a profile at each step. Their remaining tricks (the relic plan,
frozen dispatch tables, fused `legal_actions`) only become visible once the listener rebuilds are gone.

## Verification, every step

1. `cargo xtask bench --mode fights` (2200×50) prints digest `ad55be60f5bd8a71`, wins 35,868 and actions
   2,412,377. `--mode runs` (200) prints digest `4a353e75d1d6e2b9`. If either changes, the step changed
   behaviour; revert or explain.
2. `cargo xtask gate`.
3. `tools/regress.sh` (every encounter and loadout against the referee, `regress` profile with debug
   assertions, which run the listener oracle and `REPLAY_CHECK`).
4. `tools/regress_runs.sh` for whole runs.
5. `cargo xtask sweep` must print a table identical to the one before the step.
6. For #4, also the map digest test and `tools/diff_map.sh`.
7. Re-measure with `taskset`, ABAB against the previous build, and record the numbers in
   `docs/perf.md`.

## Credit

No spire-mentor code is copied by this task. If #4b or other code is taken from
`spire-mentor/sts2-rs` (MIT, © Thomas Friedel), add a `NOTICE` entry and a header comment naming the
source file.
