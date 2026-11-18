# RLN27: net-guided lookahead in act-2 boss fights

> Superseded by `rln28-honest-boss-lookahead.md`: with the leak closed the same lookahead loses
> to greedy (-4.0 pp on Kaiser Crab, -8.8 pp over 600 fights).

**Verdict: inconclusive; the measured gain is not a usable-teacher result.** The lookahead beats greedy
by +17.5 pp on Kaiser Crab (CIs do not overlap), but the engine's mid-fight copy keeps the fight's
random streams and the draw-pile order, so every rollout previews the real future. Wins from that
leak cannot be taught to a net that does not see it. A fair test needs a core change (below).

## Method
`python/rln27_lookahead.py run`: on the 600 fixed fights of `python/rln19/act2-boss-bench-v2.jsonl.gz`
(1B pin, `ema0.999`), at each agent decision take the net's top-k=4 actions, copy the fight state
n=8 times per action, apply the action, play to the end with the net sampling at temperature 1, and
play the action with most wins, then most HP left (deaths 0).
`--k 0` is the greedy baseline through the same code (reproduces 17.5 / 36.0 / 62.5 %).

State copy: `RunEnvs::copy_env` (`sts2-rl`, FFI `sts2_run_envs_copy`, `Sts2RunEnvs.copy`). It clones
the `Run` (all streams included) and counters, then re-runs `settle` so obs and mask are exactly what a
step writes. Exact and fast (no prefix replay). Test: `python/tests/test_rln27_copy.py` (copies
played with the same random legal actions match obs, mask, events and end at every step; 3 bosses).

## Result (k=4, n=8)
| boss | fights | greedy wins (Wilson 95%) | lookahead wins (95%) | diff pp (paired 95%) | both | greedy-only | search-only | neither | HP left greedy / search |
|---|---:|---|---|---|---:|---:|---:|---:|---|
| Kaiser Crab | 200 | 35 (17.5%, 12.9-23.4) | 70 (35.0%, 28.7-41.8) | +17.5 (+12.2 to +22.9) | 35 | 0 | 35 | 130 | 3.1 / 9.7 |
| The Insatiable | 200 | 72 (36.0%, 29.7-42.9) | 116 (58.0%, 51.1-64.6) | +22.0 (+16.1 to +27.6) | 72 | 0 | 44 | 84 | 10.0 / 18.8 |
| Knowledge Demon | 200 | 125 (62.5%, 55.6-68.9) | 169 (84.5%, 78.8-88.9) | +22.0 (+16.2 to +27.8) | 125 | 0 | 44 | 31 | 19.6 / 32.7 |
| All | 600 | 232 (38.7%, 34.9-42.6) | 355 (59.2%, 55.2-63.0) | +20.5 (+17.2 to +23.7) | 232 | 0 | 123 | 245 | 10.9 / 20.4 |

HP left: mean over fights, deaths count 0 (run HP after the boss dies). Speed: 24,931 agent
decisions in 1,665 s mean per-process wall (three processes, one per boss, 1 env thread each, so about
15 decisions/s per process, 45/s together at 33 envs per fight); 2,037 s for the slowest. Whole job 34 min.
Greedy-only is 0 in all 600 fights: a fair search would lose some fights greedy wins; this is the sign of the leak.

## Why the result is leaky
The copy shares the combat RNG state, the monsters' own streams and the (already shuffled) draw pile.
A rollout of the same actions therefore sees the real draws and enemy rolls: perfect-information
play. The engine reseeds streams only inside previews (`combat/preview.rs`, `pub(super)`), and
that does not reshuffle the draw pile.

## Blocked: needs a core change (not made; the brief forbids editing `sts2-core`)
A public `Combat` method that determinizes a copy: reseed the combat streams and every monster's
stream from a seed (as `preview::reseed`), and shuffle the draw pile with the new stream (the order is
hidden from the player). `copy_env` would then take a seed per copy. With it, rerun
`python/rln27_lookahead.py run ... --k 4 --n 8`: about 35 min for all 600 fights on three single-thread
processes. The verdict rule (>= 10 pp on Kaiser Crab, non-overlapping CIs) applies to that run.
No larger budget was run: the k=4, n=8 gain is not trustworthy, so a bigger budget would only enlarge the leak.

Files (scratch, not committed): `target/rln27/{greedy,k4n8}.jsonl.gz`.
