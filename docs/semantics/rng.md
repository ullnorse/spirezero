# Semantics: RNG, decimal and shuffle

Append-only. One fact per bullet, with a decomp reference (`ref/<path>:<line>`) and the task id that found it. Index: `docs/semantics.md`.

## RNG, decimal and shuffle (R2)
Full detail in `docs/specs/rng-decimal.md`. Paths are under `ref/decomp/`.
- `Rng` = xoshiro256** seeded by 4x splitmix64; every public method makes exactly one `NextDouble()` draw (`(u64>>11)*2^-53`). `NextInt(max)` is `(int)(NextDouble()*max)`, no rejection sampling; `NextBool` is `NextInt(2)==0`; `NextFloat` goes through the f64 path. `MegaRandom.NextBool/NextFloat/NextInt()` are never used by `Rng`. (R2) `MegaCrit.Sts2.Core.Random/Rng.cs:72-190`, `MegaCrit.Sts2.Core.Random/MegaRandom.cs:87-105,168-237`
- Stream seed = `xxh64(utf8(run_seed_string)) + xxh64(snake_case(RunRngType name))` (u64 wrapping). The name comes from the enum member (`CombatOrbs`), not the property (`CombatOrbGeneration`). Player streams use `xxh64(seed)+slot` as the base and names `rewards`, `shops`, `transformations`. (R2) `MegaCrit.Sts2.Core.Runs/RunRngSet.cs:115,139-143`, `MegaCrit.Sts2.Core.Random/PlayerRngSet.cs:236-240`, `MegaCrit.Sts2.Core.Entities.Players/Player.cs:328-331`
- Seed string: upper-case, `O->0`, `I->1`, trim; `Seed = XxHash64(UTF-8, seed 0)`. (R2) `MegaCrit.Sts2.Core.Helpers/SeedHelper.cs:36-43`, `MegaCrit.Sts2.Core.Helpers/StringHelper.cs:139-153`
- `UnstableShuffle` is Fisher-Yates from the end with `j=NextInt(n+1)` (`len-1` draws) and depends on input order (initial draw pile). `StableShuffle` (reshuffle and many card effects) first runs `List<T>.Sort()` (unstable introsort; ties = same id and upgrade level) and then the same shuffle. (R2) `MegaCrit.Sts2.Core.Extensions/ListExtensions.cs:22-60`, `MegaCrit.Sts2.Core.Models/CardModel.cs:2272-2294`
- `CardPilePosition.Random` insertion draws `Shuffle.NextInt(count+1)`, even for an empty pile. (R2) `MegaCrit.Sts2.Core.Commands/CardPileCmd.cs:509`
- Monster AI random branches use f32 weights and `rng.NextFloat(sum)`, so f32 emulation is required in the RNG module. (R2) `MegaCrit.Sts2.Core.MonsterMoves.MonsterMoveStateMachine/RandomBranchState.cs:115-127`
- Non-integer decimals come from literals (0.75, 1.5, 0.1, 0.5, 0.3, 0.25, ...), `/100m` percents and `Math.Pow(2, n)`. The `(int)` cast on `decimal` truncates and throws on overflow. A fixed-scale `i64` is not enough; use an exact `i128` mantissa plus scale. (R2) `MegaCrit.Sts2.Core.Models.Powers/WeakPower.cs:23`, `MegaCrit.Sts2.Core.Models.Powers/ShadowmeldPower.cs:30`, `MegaCrit.Sts2.Core.Hooks/Hook.cs:2525-2570`

## RNG corrections (T2)
- A throwing `NextInt(0)` or `NextInt(-1)` still increments the counter without advancing the state. `NextInt(5,5)` throws before incrementing. Source: golden fixtures `rng.json` (T2).
- `WeightedNextItem` sums its weights with `Enumerable.Sum(Func<T,float>)`, which may accumulate in `double`, not f32 as `rng-decimal.md` says. Port it against `weighted_static` in `rng.json`. (T2, UNVERIFIED)
- Real saved-run seeds are 10 characters (e.g. `94LYYA8YDC`); `CanonicalizeSeed` cases are in `seed_hash.json`. (T2)
- Gaussian outputs depend on the host's `Math.Log/Cos/Sin`, so the last bit may differ across platforms. Avoid relying on them. (T2)

## RNG/decimal port notes (C0)
- `WeightedNextItem`: `rng.json` gives identical picks whether the weight total is summed in `f32` or `f64`, so the fixtures don't settle T2's question. The port sums in `f64` (LINQ `Sum(Func<T,float>)`) and keeps it UNVERIFIED. Source: `crates/sts2-core/tests/golden_rng.rs` (C0)
- `NextFloat(min, max)` computes `max - min` in `f32`, so `(-3.4e38, 3.4e38)` overflows to `+inf` and every output is `+inf`. Matches `rng.json`. (C0) `MegaCrit.Sts2.Core.Random/Rng.cs:167-190`
- `NextUnsignedLong(min, max)` is public as well, not only `(max)`; it goes through `f64` in the same way. (C0) `MegaCrit.Sts2.Core.Random/Rng.cs:140-165`
- `CanonicalizeSeed` uses one-to-one upper-casing (`Straße` -> `STRAßE`, not `STRASSE`). Source: `seed_hash.json` (C0)
- The `SnakeCase` regex is equivalent to "insert `_` before an ASCII capital that follows an ASCII letter or digit" (the `\G` branch only continues a run of capitals). Source: `seed_hash.json` (C0)
- The introsort's `HeapSort` fallback (depth limit reached) is not hit by any fixture, so its tie order is unverified. (C0)
