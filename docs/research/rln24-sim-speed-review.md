# RLN24: speed review of the simulator and the RL env path (review only, no code changed)

**Workload.** `target/fast/sts2-bench --mode run-env --threads 1 --envs 64 --runs 400 --e2e --obs-v2`
(the RLN22 cost workload: Necrobinder A0, uniformly random legal play, 400 runs, 18,173 agent
decisions of which 15,370 are combat decisions, 3,102 auto-taken single options, digest
`9c2413f37b7191da` in all three variants). Callgrind, `fast` profile with line tables, no
`target-cpu=native`, `nice -n 10 taskset -c 14,15` beside the live lane; no wall-clock numbers.
Caveat: random play dies at floor ~4, so run starts (`Run::new`, map generation) are
over-represented and deep fights (more cards, enemies and powers) under-represented against
NECRO1; the per-call costs transfer, the shares shift towards the combat items.

## Where the instructions go

| variant | Ir total | Ir / decision | delta |
|---|---:|---:|---|
| `--e2e` (v1 obs) | 777.2 M | 42.8k | |
| `--e2e --obs-v2 --no-plan` | 1,106.9 M | 60.9k | obs_v2 without the plan column: +18.1k / decision |
| `--e2e --obs-v2` (NECRO1's config) | 2,883.0 M | 158.6k | plan column: +97.7k / decision, +115.6k per combat decision |

Inclusive shares of the full config (2,883 M), per call where it matters:

| part | Ir | share | calls | Ir / call | note |
|---|---:|---:|---:|---:|---|
| `plan_column` (`plan_column.rs`) | 1,727 M | 59.9% | 15,370 | 112k | depth 1, one search per combat decision |
| ├ `search_turn` (`resolve.rs:235`) | 1,455 M | 50.5% | 15,207 | 95.7k | |
| │ ├ end-turn simulation (`turn.rs`: `start_turn` 491 M, enemy turn 358 M) | ~575 M | 20% | | | inherent to scoring at the next turn start |
| │ ├ `Combat::clone` + `CombatState::clone` from the search | 419 M | 14.5% | 123,771 | 3.4k | 8.1 fresh copies per decision |
| │ └ `drop_glue::<Combat>` and the beam `Vec`s' drops | ~90 M | 3.1% | 140,646 | 0.7k | |
| ├ `blindfold_for_search` (`combat/mod.rs:945`) | 243 M | 8.4% | 15,370 | 15.8k | |
| │ └ `RunRngSet::new` (`rng/sets.rs:134`, `snake_case` 127 M) | 162 M | 5.6% | 15,370 | 10.5k | the same seed string every time |
| allocator (mimalloc malloc/free/realloc, self) | ~300 M | ~10% | | | almost all from the fresh copies above |
| `memcpy` (self) | 349 M | 12.1% | | | copies and obs writes |
| `IntentCache::lookup` (`intents.rs`) | 250 M | 8.7% | 79,904 | 3.1k | 37,216 misses (47%) at 5.4k each = 200 M |
| ├ forecast lookups (obs_v2, 3 steps × enemy) | 185 M | 6.4% | 59,922 | | |
| └ current intents (v1 section) | 65 M | 2.2% | 19,982 | | |
| `run_obs` v1 sections | 255 M | 8.8% | 18,173 | 14.0k | |
| └ `run_obs::map` (`run_obs.rs:305`) | 133 M | 4.6% | 18,173 | 7.3k | rewritten at every decision, changes only on map moves |
| `Run::accepts` (`run/accepts.rs:19`) | 140 M | 4.9% | 104,736 | 1.3k | 6,323 slow calls (`clone` + `apply_unjournaled`) at 19.3k = 122 M |
| `card_preview` (obs_v2 previews, `obs_v2.rs:251/279`) | 97 M | 3.4% | 104,662 | 0.9k | 6.8 calls per combat decision |
| `Observer::observe` less intents | 33 M | 1.1% | 15,370 | 2.1k | |
| `RunEnv::apply` → `Run::apply` (the real engine step) | 142 M | 4.9% | 21,275 | 6.7k | already optimised (`docs/perf.md`) |
| `reset_fresh` → `Run::new` (map 84 M) | 155 M | 5.4% | 400 | 388k | per run; under 1% at NECRO1's floors |
| dedupe (`classes.rs`, `same_card`) | ~35 M | 1.2% | | | |
| `legal_actions_into`, mask fill | ~32 M | 1.1% | | | |

## How much of collect this is

The env's thread time is not measured here (no wall clock allowed). From RLN22's numbers: the plan
column cost +9.1 thread-µs per decision on this bench, and at full scale collect went from 26.6k to
24.0k dps, which is +4.4 wall-µs per decision. With 14 threads fully used the expected rise was
+0.65 wall-µs. The factor of ~7 is the A10 fights being bigger (RLN22 measured 30 µs at boss
fights, 3.3×) and the env step's parallel efficiency being poor (see finding 7). Taking the plan
column as ~12% of collect wall and 60% of the env's instructions, the whole Rust env is of the
order of 20% of collect wall, ~10% of an iteration; that is the ceiling for Rust-only work. The
trainer's collect loop (`python/train_e2e.py`, `_rollout`) is strictly sequential: obs copy →
forward → sample → `.cpu()` → `envs.step` → next; nothing overlaps the env step with the GPU.
First action of any speed lane: time `envs.step` inside the trainer for one iteration.

## Findings, ranked by expected gain per hour

Gains are the share of the env's instructions on this workload (the whole Rust path, plan column
on); risk is to engine parity and obs bytes.

| # | finding | where | cost now | change | gain | work | risk |
|---|---|---|---:|---|---:|---:|---|
| 1 | `blindfold_for_search` builds `RunRngSet::new(&format!("SEARCH{seed}"))` on every call; the seed is the constant `COLUMN_SEED`, so the string formatting, xxh64 and the per-stream `snake_case` names are recomputed 15k times for the same result | `combat/mod.rs:945-952`, `plan_column.rs:26` | 162 M, 10.5k per combat decision | build the set once (`LazyLock<RunRngSet>` keyed by seed, or a `blindfold_for_search_with(&RunRngSet)` the caller feeds from a cached set) and `clone` it; also hand the blindfolded copy to `search_turn` by value so its `combat.clone()` for the beam root (15k copies, ~50 M) goes | 5.6% (+1.7%) | 0.5–1 h | none: same values; a tiny core edit |
| 2 | `search_turn` makes 2L+2 fresh `Combat` clones per decision (baseline `combat.clone()`, beam root, `state.clone()` per action, then `copy.clone()` again to end the turn) and drops them all; at `OBS_DEPTH` = 1 the kept `copy` in `next` is never stepped again, so the second clone per action and the `Vec<(Combat, …)>` are pure waste; `first.to_vec()` and `plan.clone()` allocate per action | `resolve.rs:246-258, 279, 306-309` | 419 M of clones + ~90 M drops + most of the allocator's 300 M; 27k+6k per combat decision | one scratch `Combat` per depth refilled with `clone_from` (the S4A′ pattern, allocation-free after warm-up); when `depth + 1 == max_plays` end the turn on `copy` itself and keep only `(score, root)`; keep plans as indices into `first` | 15–20% | 3–4 h | low: `clone_from` is asserted equal to `clone` in debug/regress builds; gate = Greedy benchmark 22/600 fight by fight, run-env digest, `tests/plan.rs`, `tests/leak.rs` |
| 3 | The intent cache misses 47% of lookups; each miss runs the move on a scratch copy (5.4k). Forecast entries are keyed per (creature, step) with the whole `Monster` (machine position, RNG, roll flags) in the key, so every new roll invalidates all 3 steps although the forecast of move `at` is the same move as last turn's step +1 | `intents.rs:150-200`, `obs_v2.rs:135` | 200 M in misses; 16k per combat decision for all intent work | look a forecast move up by its `at` among the creature's entries (a small per-creature map `StateIdx → Entry`) instead of by step slot; where the move reads no RNG, drop `monster.rng` and the machine position from its key (the Token/Listeners/Full key choice stays) | 4–6% | 3–5 h | medium: the debug oracle (`debug_assert_eq!` against the slow path) and `python/obs_v2_digest.py` over 100k decisions must stay green |
| 4 | The map section (96 points × 5 bytes, two `Grid`s, `column_bit` with checked shifts per point) is rewritten at every decision; it changes only when `act_index`, `visited.len()`, the coord or the pending decision kind (`Map` or not) change | `run_obs.rs:305-333` | 133 M, 7.3k per decision | keep the last 480 bytes and their key in `RunObserver`; `copy_from_slice` on a hit | 4.6% | 1–2 h | low: bytes identical by construction; run-env digest + `tests/leak.rs` |
| 5 | `Run::accepts` falls back to `self.clone().apply_unjournaled(action)` for every listed run-level action not in `surely_accepts`; 2.3 slow calls per non-combat decision here, 19.3k each (a `Run` clone is 6k, the apply 13k); with deeper decks, 16-item shops and 20-option selections at NECRO1 the slow path grows | `run/accepts.rs:19-53`, called from `run_action.rs:173` and `classes.rs:302` | 122 M, 4.2% | core ask: widen `surely_accepts` to the actions that cannot hit unported content (card selections, confirm/cancel, rest options, skip/claim of ported relics, map moves into ported encounters/events: the engine knows the room's content before entering), leaving the clone for the rest | 3–4% | 2–4 h, core lane | medium: `check_run_mask` (debug) and the run-env digest catch a mask change; a wrongly "sure" action would end a run as `Stuck`/`Unsupported` instead of being masked |
| 6 | `card_preview` is called once per hand card plus once per live enemy per damage card, every decision; hand, costs and powers rarely change between the decisions of one turn | `obs_v2.rs:251, 279` | 97 M, 6.3k per combat decision | cache per hand slot keyed by (card id, listener generation, energy, enemies' HP/block/powers hash) in `V2Observer`; or, cheaper, compute `preview` and `aimed` for all targets from one `card_preview` call per target set if `vars.rs` exposes the per-target fold | 2–3% | 3 h | medium: obs_v2 digest |
| 7 | Multi-thread structure: `each_env` splits the envs into `threads` contiguous static chunks and spawns `threads` OS threads per step (`thread::scope`), so a step takes as long as its slowest chunk; env work is very uneven (a reset is 388k, a map decision 20k, a boss combat decision ~150k+). And the trainer never overlaps the env step with the GPU forward | `run_envs.rs:803-860`, `python/train_e2e.py:_rollout` | unmeasured; the RLN22 discrepancy above suggests an effective parallelism well under 14 | (a) work-stealing: an `AtomicUsize` cursor handing out blocks of 8–16 envs to a persistent pool (`Counts` is a commutative sum; snapshots are collected in env order afterwards, so results do not change); (b) in Python, two half-batches: `step(A)` on the Rust threads while `forward(B)` runs on the GPU | (a) up to the imbalance, (b) hides most of the env step | (a) 1–2 h, (b) 1–2 days in the trainer | (a) none for parity (digests are thread-count independent today and stay so); (b) changes the trainer's batching, not the env |
| 8 | `settle` runs the full mask pipeline (`legal_actions_into`, `fill_run`, `retain_accepted` with its `accepts` calls, dedupe with `same_card`) at every auto-taken single-option position (3,102 here, 17% of decisions) before noticing the one option | `run_envs.rs:645-700` | ~1–2% | when `legal.len() == 1` and the one action is accepted, skip dedupe and the counts (they are overwritten by the next position anyway), unless `practice_stop` | 1–2% | 1 h | low: digest |
| 9 | `write_plan` collects `legal: Vec<Action>` and `plan_column` allocates `scores` per decision; `plan_actions` clones each `Action` | `plan_column.rs:195-197`, `run_envs.rs:689-697` | <1% | pass `&[(usize, Action)]` through and keep the scores `Vec` in the env | <1% | 0.5 h | none |

Not worth a task: `Writer::new`'s zero fills (memset 15 M, 0.5%), `saturation`, `Counts` sums,
the ffi boundary (zero-copy views; the Python side copies 8.4 MB of obs per step with `.copy()`
before `.to(dev)`, cheap next to the forward pass), `legal_actions_into` and the mask fill
(1.1% together), `Run::new` (per run, not per decision).

**Order.** 1 then 2 together (one task, both in `search_turn`'s territory; about a third of the
env's instructions, measured as 15k Ir per combat decision for 1 and ~30k for 2 against 158k
now), gate with the RLN22 benchmark and the digests. Then 7(a) and the trainer-side timing of
`envs.step`, which decides whether 7(b) is worth its day. 3, 4, 5, 6 are each a few percent and
independent; 5 needs the core lane.
