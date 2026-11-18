# RLN28: an honest lookahead on the act-2 boss benchmark

**Verdict: not a usable teacher.** With the hidden future hidden, the k=4, n=8 lookahead loses to
greedy on every boss: -4.0 pp on Kaiser Crab (paired 95% -9.1 to +0.9), -8.8 pp over all 600 fights
(-12.1 to -5.5). RLN27's +17.5 pp came from the leak. The k=4, n=32 run was not made (the brief
asks for it only if n=8 is clearly ahead).

## The fix (sts2-core)
`Combat::determinize(seed)` (`crates/sts2-core/src/combat/mod.rs`), reached through
`Run::determinize_combat`, `RunEnvs::copy_env(src, dst, reseed)`, FFI `sts2_run_envs_copy(..., seeds, n)`
and `Sts2RunEnvs.copy(src, dst, reseed=seeds)`. In place, on a copy:
- the draw pile is put in card order, then shuffled with a stream from the seed (contents kept);
- reseeded (`RunRngSet.cs` says what each decides): `Shuffle`, the `Combat*` streams (card,
  potion and orb generation, card selection, energy costs, targets), `MonsterAi`, `Niche`, each
  monster's own `MonsterModel.Rng`, and the set's seed (later summons derive their stream from it);
- kept: `UpFront`, `UnknownMapPoint`, `TreasureRoomRelics` (no combat draws on them);
- unchanged: hand, discard and exhaust piles in order, HP, Block, powers, energy, relics, potions,
  the intents shown (a monster's next move is stored, not re-rolled).

Limits: a card the player saw put on top of the draw pile is shuffled too (the copy knows less than
the player); post-fight seed-derived rolls (events, `FurCoat`) differ in the copy.
The older `Combat::blindfold_for_search` also shuffles the discard pile, which the player sees, so it
was not reused. Engine behaviour is unchanged (`cargo test -p sts2-core` passes, 540 tests).

Tests: `combat/tests/rln28.rs` (same seed gives the same state, other seeds other streams; visible
state and draw contents equal, out-of-combat streams kept; the draw order changes);
`python/tests/test_rln28_reseed.py` (a reseeded copy has the source's obs and mask; two copies with
the same seed play identically; other seeds diverge); `test_rln27_copy.py` still passes.

## Result (k=4, n=8, every rollout copy reseeded; real fight on its own streams)
Same bench, checkpoint (`NECRO1/pinned/state-1B.pt`, `ema0.999`), selection rule and greedy file
(`target/rln27/greedy.jsonl.gz`) as RLN27. `python/rln27_lookahead.py run ... --k 4 --n 8`
(`--leaky` gives RLN27's copies).

| boss | fights | greedy wins (Wilson 95%) | lookahead wins (95%) | diff pp (paired 95%) | both | greedy-only | search-only | neither | HP left greedy / search |
|---|---:|---|---|---|---:|---:|---:|---:|---|
| Kaiser Crab | 200 | 35 (17.5%, 12.9-23.4) | 27 (13.5%, 9.4-18.9) | -4.0 (-9.1 to +0.9) | 19 | 16 | 8 | 157 | 3.1 / 2.7 |
| The Insatiable | 200 | 72 (36.0%, 29.7-42.9) | 53 (26.5%, 20.9-33.0) | -9.5 (-15.4 to -3.6) | 44 | 28 | 9 | 119 | 10.0 / 6.8 |
| Knowledge Demon | 200 | 125 (62.5%, 55.6-68.9) | 99 (49.5%, 42.6-56.4) | -13.0 (-19.2 to -6.6) | 90 | 35 | 9 | 66 | 19.6 / 14.4 |
| All | 600 | 232 (38.7%, 34.9-42.6) | 179 (29.8%, 26.3-33.6) | -8.8 (-12.1 to -5.5) | 153 | 79 | 26 | 342 | 10.9 / 7.9 |

Sanity: greedy-only wins are 79 of 600 (RLN27: 0), so the rollouts no longer see the real future.
Speed: 24,311 agent decisions; three processes (one per boss, 1 thread each, `nice -n 10`, GPU
3 x 0.55 GB) took 2,799 / 1,791 / 3,251 s.

## Reading
A rollout plays the net sampled at temperature 1 from the copied state, so it scores an action by
how a weaker, noisier player does after it, over 8 samples. Without the leak that score ranks
the net's own top actions worse than the net's greedy choice does. Next options, if search is
pursued: greedy or low-temperature rollouts, a value-net leaf instead of playing to the end, or
many more rollouts per action; each needs its own paired run against this greedy file.

Files (scratch, not committed): `target/rln28/k4n8*.jsonl.gz` and logs.
