# RNG, decimal and shuffle spec (R2)

Paths are under `ref/decomp/`. Abbreviations: `RNG` = `MegaCrit.Sts2.Core.Random/Rng.cs`, `MR` = `.../Random/MegaRandom.cs`,
`RRS` = `MegaCrit.Sts2.Core.Runs/RunRngSet.cs`, `PRS` = `.../Random/PlayerRngSet.cs`, `SH` = `MegaCrit.Sts2.Core.Helpers/StringHelper.cs`,
`LE` = `MegaCrit.Sts2.Core.Extensions/ListExtensions.cs`, `CPC` = `MegaCrit.Sts2.Core.Commands/CardPileCmd.cs`. `UNVERIFIED` = not provable from `ref/` (BCL source is not in the decomp).
Runtime: `net9.0`, self-contained .NET 9.0.7 (`sts2.runtimeconfig.json` `includedFrameworks`); `sts2.csproj` has `CheckForOverflowUnderflow=False`, so all C# arithmetic **wraps** (except `decimal`, which throws).

## 1. The RNG algorithm

### 1.1 State and core generator (`MR`)
`Rng` (`RNG:14`) = `MegaRandom` (xoshiro256\*\*, `MR:51`) + `int _counter` (`RNG:16`). `_counter` is bookkeeping only (saved, never feeds output).
State = four `u64` `s0..s3`. Seeding (`MR:99-105`): `x = seed; s0 = splitmix64(&x); s1 = ..; s2 = ..; s3 = ..` (four successive calls).
```
splitmix64(x): x += 0x9E3779B97F4A7C15; z = x; z = (z ^ z>>30) * 0xBF58476D1CE4E5B9; z = (z ^ z>>27) * 0x94D049BB133111EB; return z ^ z>>31   (MR:87-93, wrapping)
next_u64():  result = rotl(s1 * 5, 7) * 9;  t = s1 << 17;  s2 ^= s0; s3 ^= s1; s1 ^= s2; s0 ^= s3; s2 ^= t; s3 = rotl(s3, 45);  return result   (MR:168-187)
```
(Standard xoshiro256\*\*.) `NextBytes` (`MR:122`) is not used by `Rng`. The all-zero state cannot occur from splitmix seeding.
`NextDouble()` = `(next_u64() >> 11) as f64 * 1.1102230246251565E-16` (= 2^-53) (`MR:236`), range [0,1), **exactly 1 raw draw**.

### 1.2 Output methods of `Rng` (what game code calls). Every call does `_counter++` once, even for rejection loops' inner calls.
Everything funnels through `NextDouble()` (one raw draw each). **`MegaRandom.NextBool/NextFloat/NextInt()/NextUInt` are never used by `Rng`** (trap: don't port the "obvious" bit-tests).
| Method | Result | Raw draws | Ref |
|---|---|---|---|
| `NextInt(max)` (default `i32::MAX`) | `(int)(NextDouble() * (double)max)`; `max<1` throws | 1 | `RNG:83`, `MR:195,296` |
| `NextInt(min,max)` | throws if `min>=max`; `n=(long)max-min`; if `n<=i32::MAX` `(int)(NextDouble()*(double)(int)n) + min` else `(int)((long)(NextDouble()*(double)n) + min)` | 1 | `RNG:95`, `MR:214-226,296-304` |
| `NextBool()` | `NextInt(2)==0` (i.e. `NextDouble() < 0.5`) | 1 | `RNG:72` |
| `NextDouble()` | as 1.1 | 1 | `RNG:192` |
| `NextDouble(min,max)` | `NextDouble()*(max-min)+min` (f64) | 1 | `RNG:202` |
| `NextFloat(max=1f)` / `NextFloat(min,max)` | `(float)( NextDouble() * (double)(max - min) + (double)min )`; **`max-min` is an f32 subtraction**, then widened, product/sum in f64, one final round-to-nearest-even f64→f32 | 1 | `RNG:167-190` |
| `NextUnsignedInt(max)` / `(min,max)` | `min + (uint)(NextDouble() * (double)(max-min))` (`max-min` is `uint` wrap-free since `min<max`, then to f64) | 1 | `RNG:110-132` |
| `NextUnsignedLong()` | raw `next_u64()` | 1 | `RNG:134` |
| `NextUnsignedLong(max)` | `max==u64::MAX` → raw; else `(ulong)(NextDouble()*(double)(max))` (f64 precision loss above 2^53) | 1 | `RNG:140-165` |
| `NextItem(seq)` | empty → `default` with **no draw**; else `seq[NextInt(0, count)]` (same draw as `NextInt(count)`) | 0 or 1 | `RNG:289-299` |
| `WeightedNextItem(items, w)` | `r = NextFloat()` (1 draw, **before** weights are read); `rem = r * sum_f32(w)`; iterate `rem -= w_i; if rem <= 0 return item`; else fallback `default`. All f32 | 1 | `RNG:308-337` |
| `Shuffle(list)` (instance) | see §3.1 (same as `UnstableShuffle`) | `len-1` (0 if len<=1) | `RNG:342` |
| `NextGaussianDouble/Int` | rejection loop, 2 `NextDouble` per attempt, uses `Math.Sqrt/Log/Cos/Sin`, `Math.Round` (ties-even) | 2×attempts | `RNG:227,260` |
Gaussians are used for map generation only (rest-site counts etc., e.g. `Models.Acts/Overgrowth.cs:136`, `Map/MapPointTypeCounts.cs:32`); transcendental functions are libm-dependent → exact port `UNVERIFIED`; not needed for combat.
`GrabBag.GrabIndex` (`Helpers/GrabBag.cs:111-122`): `NextDouble() * total_weight` (f64) then cumulative `num < acc`, returns -1 if none. It is rejection-looped with a predicate (`GrabBag.cs:96-109`).
Float-in-game-logic reminder: the project rule "no floats" **cannot hold for RNG consumers**: monster AI `RandomBranchState.GetNextState` (`MonsterMoves.MonsterMoveStateMachine/RandomBranchState.cs:115-127`) does `max = Σ f32 weights; num = rng.NextFloat(max); loop num -= w; if num <= 0f`, using f32 arithmetic (weights from `GetStateWeight`, `:129+`). Port with `f32` (Rust `f32` is IEEE-754 identical for `+ - *` and casts) and round-to-nearest-even; keep it isolated in an `rng` module with an explicit `#[allow]` reason.

### 1.3 Construction / serialization
* `new Rng(seed)`: `MegaRandom(seed)` (`RNG:27`). `new Rng(seed, name)` = `new Rng(seed.wrapping_add(xxh64(name)))` (`RNG:54`, §5.1).
* `new Rng(Player, ModelId id, mixin)` = `seed = run_seed + slot_index + xxh64(id.Entry) + mixin`, computed as `(ulong)((long)a+(long)b+(long)c) + mixin` (wrapping) (`RNG:49`); `run_seed = RunState.Rng.Seed`, `slot_index = Players.IndexOf(player)` (`RunState.cs:354`). Used by relics (`FurCoat.cs:90`, `Byrdpip.cs:48`, `PaelsLegion.cs:122`).
* Save format `SerializableRng` = `{counter:i32, s0..s3:u64}` (`Saves/SerializableRng.cs:12-24`); restore sets `_counter` and `s0..s3` verbatim (`RNG:32,63`, `MR:110`). `counter` is **not** replayed, so a Rust port must restore the 4 words; `counter` is optional for correctness but cheap to keep for trace parity.
* `Rng.Chaotic` (`RNG:25`) is wall-clock seeded; cosmetic only (R1 §6). Never port.

## 2. Streams

### 2.1 `RunRngSet` (`RRS`) — run-scoped, created eagerly for every enum member, persisted with the run (so state carries across combats and floors)
Enum order/identity `RunRngType` (`Entities.Rngs/RunRngType.cs`). Stream seed = `Seed + xxh64(utf8(name))` (u64 wrapping, `RRS:139-143` → `RNG:54`), `name = SnakeCase(enum.ToString())` (§5.2). `Seed = xxh64(utf8(seed_string))` (`RRS:115`, `SH:139-153`).
| Enum member | Stream name (string hashed) | Property | Purpose (decomp doc comment) |
|---|---|---|---|
| `UpFront` | `up_front` | `UpFront` | monsters/events/relics decided at run start (`RRS:34`) |
| `Shuffle` | `shuffle` | `Shuffle` | initial draw-pile order, reshuffles, `CardPilePosition.Random` insertion (`RRS:40`) |
| `UnknownMapPoint` | `unknown_map_point` | `UnknownMapPoint` | `?` room type odds (`RRS:45`, `RunOddsSet`) |
| `CombatCardGeneration` | `combat_card_generation` | `CombatCardGeneration` | cards created in combat (`RRS:51`) |
| `CombatPotionGeneration` | `combat_potion_generation` | `CombatPotionGeneration` | potions created in combat (`RRS:57`) |
| `CombatCardSelection` | `combat_card_selection` | `CombatCardSelection` | random pick among existing cards (`RRS:62`) |
| `CombatEnergyCosts` | `combat_energy_costs` | `CombatEnergyCosts` | random costs (`RRS:67`) |
| `CombatTargets` | `combat_targets` | `CombatTargets` | random targets (`RRS:72`) |
| `MonsterAi` | `monster_ai` | `MonsterAi` | monster move rolls (`RRS:77`) |
| `Niche` | `niche` | `Niche` | misc one-offs, unique monster HP at spawn (`RRS:83`, `Combat/CombatState.cs:241`) |
| `CombatOrbs` | `combat_orbs` | `CombatOrbGeneration` (**name from enum, not property**) | orbs (`RRS:88`) |
| `TreasureRoomRelics` | `treasure_room_relics` | `TreasureRoomRelics` | `RRS:93` |
Trap: the hashed name comes from the **enum member** (`CombatOrbs`), not the property (`CombatOrbGeneration`) (`RRS:141`).
Also exists: `RunRngSet(Rng)` (every stream = the same `Rng` instance; Bestiary only, `RRS:128`) and `MockRng` (tests, `RRS:184`); ignore.

### 2.2 `PlayerRngSet` (`PRS`) — per player
`PlayerRngSet(seed)` with `seed = xxh64(utf8(run_seed_string)) + slot_index` (wrapping) (`Entities.Players/Player.cs:328-331`; called from `RunState.cs:279,314,615`). Streams `Rewards`, `Shops`, `Transformations`, each `Rng(player_seed, snake_case(name))` → names `rewards`, `shops`, `transformations` (`PRS:236-240`, `Entities.Rngs/PlayerRngType.cs`). Also persisted (`PRS:242-263`). Note: the player seed uses the **string** hash plus the slot (not `RunRngSet.Seed + slot` — same value, since both are `xxh64(seed)`).

### 2.3 Other derived (non-set) RNGs relevant to the engine
| Owner | Seed | Ref |
|---|---|---|
| Monster instance `monster.Rng` | `(ulong)((long)run_seed + map_col + map_row + act_index + creature.CombatId)`; `col,row` = `CurrentMapCoord` or 0 | `Combat/CombatState.cs:245`; only consumer found in `Models.Monsters` is cosmetic skin choice (`ToughEgg.cs:118`); AI uses `MonsterAi` (`MonsterModel.cs:418`); getter returns `Chaotic` for canonical models, `MonsterModel.cs:101-108` |
| Encounter `_rng` | `(ulong)((long)run_seed + TotalFloor) + xxh64(Id.Entry)` | `Models/EncounterModel.cs:262-264` |
| Event `Rng` | `(ulong)((long)run_seed + (IsShared ? 0 : slot)) + xxh64(Id.Entry)` | `Models/EventModel.cs:234` |
| Act list | `Rng(xxh64(seed), "act_selection")` | `Multiplayer.Game.Lobby/StartRunLobby.cs:471` |
| Act map | `Rng(run_seed, "act_{act_index+1}_map")` (and `"spoils_map"`, `"map_point_selection"`, `"event_synchronizer"`) | `Map/StandardActMap.cs:113`, `SpoilsActMap.cs:92`, `MapSelectionSynchronizer.cs:48`, `EventSynchronizer.cs:77` |
Several of these cast `ulong→long` then back; with unchecked arithmetic this equals plain `wrapping_add` on u64 two's complement. Combat itself (Necrobinder) only touches the §2.1 streams (`monster.Rng` is cosmetic only).

## 3. Shuffle

### 3.1 `UnstableShuffle` (`LE:45-60`) — Fisher-Yates from the end, **order-dependent**
```
n = len;  while n > 1 { n -= 1;  j = rng.NextInt(n + 1);  swap(list[n], list[j]) }     // j in [0, n], j == n is a no-op swap but the draw still happens
```
Draws = `len-1` (0 for len 0/1). `Rng.Shuffle<T>` (`RNG:342`) is the identical loop (`for i = len-1 down to 1: j = NextInt(i+1); swap`). Used for: initial draw pile (`Entities.Cards/CardPile.cs:71` via `Player.PopulateCombatState`, `Player.cs:814`, rng = `RunState.Rng.Shuffle`, `Combat/CombatManager.cs:460`), `AggressionPower.cs:28`, `AfflictionModel.cs:244` (`CombatCardGeneration`), `IEnumerableExtensions.cs:19`, map/event/reward code.
The input order for the initial shuffle is the deck order at the time (`Deck.Cards` order, each cloned and appended: `Player.cs:806-813`), so the deck order **must** be tracked and equal to the game's.

### 3.2 `StableShuffle` (`LE:22-31`) — sort, then `UnstableShuffle`
```
tmp = list.ToList(); tmp.Sort();           // List<T>.Sort(): default comparer = T.CompareTo, NOT a stable sort
list[i] = tmp[i] for all i;  return list.UnstableShuffle(rng)
```
Same RNG draws as 3.1 (the sort consumes none). The sort only makes the shuffle independent of input order **up to ties**. For cards, `CardModel.CompareTo` (`Models/CardModel.cs:2272-2294`) = `ModelId.CompareTo` (ordinal compare of `Category` then `Entry`, `Models/ModelId.cs:42-50`, `AbstractModel.cs:96-107`) then `CurrentUpgradeLevel`. Two cards with the same id and upgrade level compare **equal** (0), regardless of enchantment/affliction/cost/instance, so their relative order after `Sort()` is the BCL introsort's business.
* **Which sort:** `List<T>.Sort()` → `Array.Sort` → `ArraySortHelper<T>.Sort` → `IntrospectiveSort` (.NET 5+, still in 9), **unstable**. `UNVERIFIED` (BCL not in `ref/`); algorithm from memory of dotnet/runtime `ArraySortHelper.cs`:
  `depthLimit = 2*(floor(log2(n))+1)`. `IntroSort(span, depth)`: loop while `len > 1`: if `len <= 16`: `len==2`→`SwapIfGreater(0,1)`; `len==3`→`SwapIfGreater(0,1),(0,2),(1,2)`; else `InsertionSort`; return. If `depth==0` → `HeapSort`, return. `depth -= 1`; `p = PickPivotAndPartition`; recurse on right part `[p+1..]`, loop on left `[..p]`.
  `SwapIfGreater(i,j)`: if `i != j && cmp(k[i], k[j]) > 0` swap. `PickPivotAndPartition`: `hi=len-1; mid=hi>>1`; `SwapIfGreater(0,mid),(0,hi),(mid,hi)`; `pivot=k[mid]`; swap `mid`↔`hi-1`; `left=0,right=hi-1`; `while left<right { while cmp(k[++left],pivot)<0 {} while cmp(pivot,k[--right])<0 {} if left>=right break; swap(left,right) }`; `if left != hi-1 swap(left,hi-1)`; return `left`. `InsertionSort`: for `i in 0..len-1 { t=k[i+1]; j=i; while j>=0 && cmp(t,k[j])<0 { k[j+1]=k[j]; j-- } k[j+1]=t }` (**stable** for len<=16). `HeapSort` is the classic `DownHeap` variant. Port exactly and validate with golden vectors (§6).
* **When ties are observable:** for lists `<= 16` insertion sort preserves input order of ties; above that, tie order is deterministic but not input-order. It only matters if tied cards differ in untracked state (enchantment, cost modifiers, instance identity used later). Cheapest exact approach: port the introsort verbatim over card instances with the game's comparator.
* **Input order of a reshuffle** (`CPC:1076-1088`): `list = discard.Cards (pile order) ++ HashSet(draw.Cards)` (`ToHashSet` of a list enumerates in insertion order — `UNVERIFIED`, no removals so true in practice); then `list.StableShuffle(RunState.Rng.Shuffle)`; then `Hook.ModifyShuffleOrder(.., isInitialShuffle:false)` (`PerfectFit` enchantment moves its card, `Models.Enchantments/PerfectFit.cs:10`; **initial shuffle also fires the hook** with `isInitialShuffle:true`, `CardPile.cs:73`); the result is appended to the draw pile via `Add(.., Bottom)`. The draw pile (post-shuffle) is thus the new list in order, top = index 0.
* Other `StableShuffle` users in combat: Seeker Strike/Power Cell/Stone Cracker (`CombatCardSelection`), Uproar/Catastrophe/Beat Down (**`Shuffle` stream**, `Cards/Uproar.cs:30`, `Catastrophe.cs:25`, `BeatDown.cs:26`), Fragrant Mushroom etc. (`Niche`). Each draws `len-1` values on the named stream (over the filtered list, not the whole pile).
* Other pile randomness: `CardPilePosition.Random` insertion index = `Shuffle.NextInt(pile.Count + 1)` (1 draw, even for an empty pile) (`CPC:509`); random draw from draw pile = `CombatCardSelection.NextItem(drawPile.Cards)` (`CPC:1163`; 0 draws if empty).

## 4. `decimal`

### 4.1 Where non-integers arise (all literals found under `Models*`, `Commands`, `Entities`, `Factories`)
* Multipliers: Weak `0.75m` (`Models.Powers/WeakPower.cs:23`), Vulnerable `1.5m` (`VulnerablePower.cs:23`), Frail `0.75m` (`FrailPower.cs:32`), Corrupted `1.5m` (`Enchantments/Corrupted.cs:26`), Surrounded `1.5m` (`SurroundedPower.cs:71`), Tank `1.5m`/`0.5m`, Guarded/Colossus/Undying Sigil `0.5m`, Slow `1m + 0.1m*stacks` (`SlowPower.cs:50`), Skittish `×(1+0.5·(players−1))` (`SkittishPower.cs:86`), multiplayer scaling `1.1/1.2/1.3m` (`Singleton/MultiplayerScalingModel.cs:75-83`), Shadowmeld `(decimal)Math.Pow(2.0, Amount)` (`ShadowmeldPower.cs:30`).
* Percent powers: `1m + (decimal)Amount/100m` (Lethality `:47`, Tracking `:37`), `amount + Amount/100m` (Cruelty `:27`), additive `+0.25m`/`-0.15m` (Paper Phrog/Krane).
* Percent HP: `(decimal)MaxHp * 0.3m` (Fairy `Potions/FairyInABottle.cs:30`, Rest `HealRestSiteOption.cs:92`), `MaxHp * pct / 100m` (Blood Potion, Ambergris, Lizard Tail `:57`, Meat on the Bone `:63` `(int)(MaxHp*(x/100m))`). `Math.Floor((decimal)n / base)` (No Escape `Cards/NoEscape.cs:29`), `(int)((decimal)n / divisor)` (Joss Paper `:131`; divisor can give a non-terminating quotient, e.g. /3).
* Sentinels: `decimal.MaxValue` = "no cap" (`AbstractModel.cs:1597`, `IntangiblePower.cs:51`, `HardToKillPower.cs:19`, `Hook.cs:2549`).
* `(decimal)float` conversions exist outside combat: `(decimal)rng.NextFloat()` (`Factories/CardFactory.cs:290`, upgrade odds), `JungleMazeAdventure.cs:67`. .NET `(decimal)float` keeps **7 significant digits**, `(decimal)double` keeps **15** (`UNVERIFIED`: BCL); only needed for rewards/events.
### 4.2 Pipeline operations (R1 §4.3, `Hook.cs:2525-2570`)
Damage: start `d`; additive pass `d += listener.Add`; multiplicative pass `d *= listener.Mul` **one factor at a time in listener order** (no pre-multiplication); cap pass `d = min(d, cap)`; `max(0, d)`. Values stay exact until an **explicit `(int)` cast** (truncate toward zero, throws `OverflowException` outside `i32`, never wraps) at: HP loss (`Creature.cs:450`), block removal `431-434`, block gain `466`, power amount `PowerCmd.cs:240`; `Math.Ceiling` for draws (`CPC:1025`); `decimal.Truncate` for detecting modifiers (`Hook.cs:1735-1765`); `Math.Floor`/`Max`/`Min`/`Clamp(…,0,999999999)`.
### 4.3 What a Rust emulation needs
Operations: `+ - *`, `/` (only by `100m` and small ints/`BaseValue`), comparisons (`<`, `>`, `!=`; **numeric**, so `1.0m == 1m`, the scale never affects results, only `ToString`), `from_i32`, `trunc`, `floor`, `ceil`, `min/max/clamp`, `to_i32` (truncate; error on overflow), `MAX`/`MIN` sentinels, `from_f64` via `Math.Pow(2,n)` (exact for `n <= 26` and cast of an integer-valued double is exact), optional `from_f32/f64` with 7/15-digit rounding.
**Verdict: a scaled `i64` is NOT enough in general; use `{ mantissa: i128, scale: u8 }` (exact decimal, no floats).** Reasons: (a) values are chained products: a capped damage of `999_999_999` (10 digits, `Creature.cs:450`) times `k` two-decimal factors needs `10+2k` digits and `i64` (18.9 digits) overflows at `k>=5` where `i64` with a fixed scale would instead have to *round*, diverging from .NET's exact result; (b) `decimal::MAX` (≈7.9e28, 96-bit) sentinel does not fit `i64`; (c) true division (`/100m`, `/3`) yields up to 28 fractional digits. `i128` holds the full 96-bit mantissa, so add/sub/mul are exact until the true decimal also loses precision. Required fidelity rule: when a result has more than 28 fractional digits or exceeds 96 bits, .NET rounds the scaled result (to-nearest, ties-to-even) / throws `OverflowException` (`UNVERIFIED` BCL detail; irrelevant for all values seen in the Necrobinder paths above, which are exact — add a `debug_assert!` that mantissa fits 96 bits and scale <= 28, and fall back to `panic` rather than silently round). Division: compute quotient to 28 fractional digits with ties-even rounding, trailing zeros trimmed for compare only; every observed consumer then truncates with `(int)`/`floor`, so truncation of the exact rational (`a*10^s / b`) matches.
Do **not** use `f64` anywhere in the damage/block/heal path: `0.1m*5` etc. are exact in decimal and inexact in binary.

## 5. Seeds

### 5.1 Hash `StringHelper.GetDeterministicHashCode` (`SH:139-153`)
`xxh64(bytes = UTF-8(str), seed = 0)` via `System.IO.Hashing.XxHash64.HashToUInt64(span, 0)` (the game ships `System.IO.Hashing.dll`). Standard XXH64 (primes P1=11400714785074694791, P2=14029467366897019727, P3=1609587929392839161, P4=9650029242287828579, P5=2870177450012600261; 32-byte stripes with 4 accumulators initialised `seed+P1+P2, seed+P2, seed, seed-P1`; merge rounds; tail 8/4/1-byte steps; avalanche `^>>33 *P2 ^>>29 *P3 ^>>32`). Known answer: `xxh64("") = 0xEF46DB3751D8E999`. Use the `xxhash-rust`/`twox-hash` crate in tests only, or hand-roll (~50 lines). Old (pre-v19) saves use `GetDeterministicHashCodeOld` (`SH:157-170`, a 32-bit Bernstein-style hash over UTF-16 code units; used only if the seed string starts with `"old"` (case-sensitive), `RRS:107-111`; migration `SerializableRunV18ToV19.cs:39,89`). Unreachable from the UI since seeds are upper-cased (§5.3); skip unless loading ancient saves.
### 5.2 `SnakeCase` (`SH:90-93`)
`Regex.Replace(txt.Trim(), @"([A-Za-z0-9]|\G(?!^))([A-Z])", "$1_$2").ToLowerInvariant()`. For the PascalCase enum names here (no consecutive capitals, no digits) this is: insert `_` before every uppercase letter except the first, lower-case all. Results listed in §2.1/2.2. Dynamic names (`"act_{n}_map"`, `"spoils_map"`) are literal strings, not snake-cased.
### 5.3 Player-visible seed → internal seed
* UI seed is a string of 12 chars from `0123456789ABCDEFGHJKLMNPQRSTUVWXYZ` (34 symbols, no `I`/`O`) (`Helpers/SeedHelper.cs:12-14`); random seeds drawn with `Rng.Chaotic.NextItem` and re-rolled if `BadWordChecker` hits (`SeedHelper.cs:16-34`) — generation is not part of the replay.
* User-typed seed: `CanonicalizeSeed` = `ToUpperInvariant`, `O→0`, `I→1`, `Trim` (`SeedHelper.cs:36-43`; applied `StartRunLobby.cs:740`). Daily runs use a date string canonicalised the same way (`NDailyRunScreen.cs:463`).
* Internal: `Seed = xxh64(utf8(canonical_string))` as `u64` (`RRS:115`); the original string is kept as `StringSeed` and saved as `seed` (`RRS:145-155`, `SerializableRunRngSet.cs:11`). To replay a saved run: read `seed` + each `rngs[type]` `{counter,s0..s3}` (dictionary keyed by `RunRngType`); `FromSave` constructs from the string then overwrites each stream's state (`RRS:158-166`). A run **started** from a seed needs only the string.
* No other transformation (no length check, no checksum) in `RunRngSet`. Max-length/validation done in UI `UNVERIFIED`.

## 6. Golden-vector plan (do not build here)
**Feasible.** `sts2.dll` is a plain managed net9.0 assembly in the game dir (`/mnt/c/games/Steam/steamapps/common/Slay the Spire 2/data_sts2_windows_x86_64/`, next to `GodotSharp.dll`, `System.IO.Hashing.dll`). Nothing in `Rng`/`MegaRandom`/`StringHelper.GetDeterministicHashCode`/`ListExtensions`/`RunRngSet` touches Godot at call time (the `Player` and `Saves` types appear only in unused ctors/methods; the CLR loads them lazily). `dotnet` is **not installed** here (`which dotnet` empty).
* Needs: .NET 9 SDK in WSL (or run the console app on the Windows side, where the game's bundled runtime 9.0.7 matches); a `net9.0` console csproj with `<Reference Include="sts2"><HintPath>…/sts2.dll</HintPath></Reference>` (+ `GodotSharp.dll`, `System.IO.Hashing.dll`, copy-local), `DOTNET_ROLL_FORWARD=Major`. Copy no decomp code; call the shipped assembly.
* Vectors to emit as JSON (committed under `tests/golden/`, generated data not decomp text): (1) `xxh64` of ~20 strings (empty, ASCII seeds, a 12-char seed, >32-byte, non-ASCII); (2) `new Rng(seed)` → first 100 outputs of each of `NextInt(max)`, `NextInt(min,max)` (incl. negative min, wide ranges), `NextBool`, `NextDouble`, `NextFloat()`, `NextFloat(a,b)`, `NextUnsignedInt`, `NextUnsignedLong`, plus final state words via `ToSerializable()`; (3) `new RunRngSet(seed)` → `ToSerializable()` for several seeds (checks every stream seed and the naming); (4) `new PlayerRngSet(xxh64(seed)+slot)`; (5) `UnstableShuffle` and `StableShuffle` over `List<T>` with a small test type `T : IComparable<T>` having `(key, tag)` and `CompareTo` on `key` only — lengths 0..40 and with heavy ties, so the **tie order of `Sort()` for >16 elements** is captured and the introsort port is verified; (6) `RandomBranchState`-style f32 weighted pick via `Rng.NextFloat(max)`; (7) pure BCL `decimal` ops (chains of `*0.75m`, `*1.5m`, `/100m`, `/3m`, `Math.Floor`, `(int)` casts, overflow cases) can be generated by the same app without `sts2.dll` since `decimal` is the BCL's.
* **Not feasible cheaply:** whole-engine goldens (damage pipeline, hooks) — `CardModel`/`ModelDb` need Godot and the game's resources; for these, use traces recorded in-game (T1/R1 trace types) rather than a console harness.
