# RLN25: where NECRO1's collect time goes, and the safe Rust fixes from RLN24

**Setup.** `train_e2e.py --resume` on one copy of NECRO1's `state.pt` (926.3M decisions; 2,048 envs,
14 threads, obs_v2, practice, compile), lane paused, `STS2_COLLECT_TIMING=1 STS2_STEP_TIMING=1`.
Every resume of the copy replays the same iterations (7068–7073), so runs compare iteration by
iteration. 7068–7069 carry torch.compile warm-up; numbers below are 7070–7072.

## Collect wall time, per iteration (131,072 decisions, ~5.8 s)

| phase | s | share | what it is |
|---|---:|---:|---|
| `forward` (host) | 3.28 | 56% | `encode` + net; the host blocks inside it (GPU forward 3.75 s) |
| `sample` (host) | 0.51 | 9% | Gumbel, log-softmax, `E.pack`, `decision_types` (GPU part 3 ms) |
| `.cpu()` wait | 0.002 | 0% | the forward has already synced |
| env `step` (Rust) | 0.85 → **0.62** | 15% → 11% | |
| practice bootstrap | 0.95 | 16% | `_practice_ends`: a separate forward for truncated practice episodes, at almost every step |
| `batch_in`, rest | 0.10 | 2% | |

The Rust env is ~15% of collect, ~7% of an iteration. The two larger levers are in the trainer:
the practice bootstrap (one batched forward at the end of the collect over the stored final
rows, or folded into the next step's forward, would save most of its 0.95 s) and the host syncs
inside `forward`.

## Env threads (`each_env`)

Before: static contiguous chunks, one per thread. Mean pass 7.2 ms wall, busiest thread 7.1 ms,
mean thread 5.6 ms: 77% efficiency. Now: blocks of 8 envs pulled from an atomic cursor: 92.5%. The
per-pass thread spawn costs ~125 µs (wall minus busiest), ~16 ms per iteration; a persistent pool
is not worth its unsafe code.

## Instructions (callgrind, RLN24 workload, `--threads 1 --envs 64 --runs 400 --e2e --obs-v2`)

| | Ir | Ir / decision |
|---|---:|---:|
| before | 2,883.1 M | 158.6k |
| after | **2,461.1 M** | **135.4k** (−14.6%) |

From `search_turn` (1,455 M → ~1,040 M): copies come from a spare list refilled with
`clone_from`, the end-of-turn scoring runs on one scratch combat, the last depth ends the turn on
the copy itself and keeps no state, and the blindfolded world moves into the search.

**Not done:**
- RLN24 finding 1 (cache the search `RunRngSet`): `RunRngSet::new` is called inside
  `Combat::blindfold_for_search` in `sts2-core`, which this lane may not edit. Core ask: a
  `blindfold_for_search_with(&self, seed, rng: &RunRngSet)` (or a cached set per seed). 162 M,
  5.6% on this workload.
- RLN24 finding 4 (cache the map section): the map can change in the middle of an act (Golden
  Compass regenerates it), so an exact key must compare the `ActMap`. `MapPoint::eq` over 96
  points costs ~5.3k Ir, nearly the 7.3k of writing the section: measured, the cache made the
  total 12.6 M worse (2,473.7 M vs 2,461.1 M) and was dropped. Core ask: a map generation counter
  in `RunState` (bumped by `generate_map`) makes the key (generation, visited, next) cheap.

## Full-scale collect, ABAB (A = main's library, B = RLN25), iterations 7070–7072

| | collect dps (median of 6) | env step s / iteration | total dps (median of 6) |
|---|---:|---:|---:|
| A | 22,825 | 0.76–0.90 | 12,779 |
| B | **23,894** (+4.7%) | 0.57–0.64 (−27%) | 12,666 |

Total dps moves with the update phase's run-to-run noise (update dps 24.8k–31.1k on the same
iteration), which is larger than the ~2% of an iteration the env step saved.

## Parity

Run-env digest `9c2413f37b7191da` at 1 and 2 threads; `obs_v2_digest.py` default
`a9c9c893…` (RLN22's) and `--decisions 1024000 --envs 512` `0249a90c…`, the same with main's
library; Greedy `boss_resolver` 22/600 and the plan-column resolver 23/600, identical per fight;
`rln19_act2.py bench` on `evalpins/state-now.pt` (ema0.999) 215/600, identical records with main's
library; `e2e_v2_identity.py` 0/2,000 rows differ (on the 500M pin: NECRO1 now has the plan
weights, which the script refuses); `tests/leak.rs`, `tests/plan.rs` and all Python tests pass.
