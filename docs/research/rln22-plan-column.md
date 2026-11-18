# RLN22 phase A: the plan column (a per-action heuristic score)

**Source.** The only scoring code in `sts2-rl` is the beam `Greedy` turn planner (`resolve.rs`;
`sts2-diff`'s greedy wraps it). `plan_column` (`crates/sts2-rl/src/plan_column.rs`) runs its
search once per combat decision and gives each legal action the best score of any plan it tried that
starts with that action (the state at the start of the next player turn against now: kills, enemy HP and
block, debuffs, own block, Osty, HP; `WIN_SCORE` for a winning play). It also gives the planner's choice as
an index for the one-hot. Potions the planner may not drink yet get the score of drinking them and
ending the turn. Card choices: take the option, finish with the first options, end the turn.
`search_turn` is `plan_turn` with an optional score sink and a depth; `Greedy` is unchanged (the
benchmark gives the same 22/600, fight by fight).

**Honesty.** The search runs on `Combat::blindfold_for_search(COLUMN_SEED)`: draw and discard
piles sorted by card id, then shuffled by an independent RNG; the run's and every monster's RNG
streams reseeded. With a fixed seed that copy is a function of the visible state alone, so draws,
random effects and next intents rolled inside the simulated turn show nothing hidden. Test
`tests/leak.rs::hidden_state_does_not_reach_the_plan_column`: 2,607 positions (31 card choices)
of random play on the 110 W1-all starts, full and short depth, two hidden-state perturbations
each (draw pile reversed and rotated, every stream reseeded): the column is identical. The control (the same
planner on the real state) changes at 72 positions, so the test can see a leak.

**Cost** (`examples/boss_resolver.rs column-x2` minus `column`, ffi profile, 1 thread):

| depth | Ir/decision (first 100 fights, crab) | wall µs/decision (600 fights, median ABAB) | share of collect* |
|---|---:|---:|---:|
| 1 (each action alone) | 347k | 30 | ~6% |
| 2 (`SHORT_DEPTH`) | 827k | 77 | ~16% |
| 3 | 1.08M | – | – |
| 10 (`FULL_DEPTH`) | 1.34M | 105 | ~22% |

\* 30k decisions/s on 14 threads is 467 thread-µs per decision; it assumes every decision is a
combat decision like these act-2 boss ones, so it is an upper bound.

**Quality** (RLN19 act-2 boss benchmark, 600 fights; the 781M net 204 = 34.0%, beam Greedy 22 = 3.7%):

| depth | planner's choice | column argmax | argmax wins the net loses |
|---|---:|---:|---:|
| 1 | 18 (3.0%) | 26 (4.3%) | 3 |
| 2 | 24 (4.0%) | 42 (7.0%) | 2 |
| 3 | 22 (3.7%) | 40 (6.7%) | – |
| 10 | 23 (3.8%) | 42 (7.0%) | 2 |

Argmax beats the planner's choice mostly by drinking potions the planner holds back. Its wins are
almost a subset of the net's (40 of 42).

## Phase B: the column in obs_v2 (ABI 10)

**Layout.** `PLAN_AT` = 700 in the v2 section, 140 combat actions × 3 bytes: `i16` score
(saturated) and flags (scored 1, planner's choice 2, winning play 4), at `OBS_DEPTH` = 1. Only
legal combat indices are written; the rest stay zero. Row bytes before it are unchanged (the
RLN21 digest of the first 3,696 bytes still matches). The trainer turns each entry into 5 inputs
(scored, chosen, wins, symlog score/10, symlog (score − best)/10) added to the action token
through a zero-init `x_plan` projection, its own optimiser group after `x_side`.

**Honesty.** `tests/leak.rs` (`hidden_state_does_not_reach_the_plan_section`): positions that
differ only in hidden state write identical plan bytes over 997 combat decisions (4,687 scored
actions). `tests/plan.rs`: entries only on masked indices, one choice, and the section changes
nothing else in the observation.

**Identity** on a copy of NECRO1 (829M decisions, ABI 9): 0 rows differ in logits or values for
raw, EMA 0.996 and 0.999 over 2,000 smoke rows, 595 side-stress and 595 plan-stress rows (all with
plan bytes); optimiser groups [46, 20, 12, 1, 1].

**Cost.** CPU (sts2-bench run-env `--e2e --obs-v2`, 1 thread, ABAB): 15.55 vs 6.44 µs per decision
(+9.1 µs); callgrind +98k Ir per decision. Full scale (resume of the NECRO1 copy, 2048 envs,
14 threads, 12 steady iterations): collect ~24.0k dps vs the lane's ~26.6–27.1k (−10%), total
~13.0k vs ~13.7–13.9k (−5%).
