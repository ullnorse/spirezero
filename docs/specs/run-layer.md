# Run layer: seed to the end of Act 1 (R4)

Source: decomp under `ref/decomp/`; refs are `path:line`. Prefixes (all under `ref/decomp/MegaCrit.Sts2.Core.`):
`RM`=Runs/RunManager.cs · `RS`=Runs/RunState.cs · `RRS`=Runs/RunRngSet.cs · `RGB`=Runs/RelicGrabBag.cs · `PL`=Entities.Players/Player.cs · `PRS`=Random/PlayerRngSet.cs ·
`CRO`=Odds/CardRarityOdds.cs · `PRO`=Odds/PotionRewardOdds.cs · `UMO`=Odds/UnknownMapPointOdds.cs · `ACT`=Models/ActModel.cs · `OG`=Models.Acts/Overgrowth.cs · `RSET`=Rooms/RoomSet.cs ·
`SAM`=Map/StandardActMap.cs · `MPP`=Map/MapPathPruning.cs · `MPO`=Map/MapPostProcessing.cs · `MPT`=Map/MapPointTypeCounts.cs · `MP`=Map/MapPoint.cs · `EVM`=Models/EventModel.cs ·
`AEM`=Models/AncientEventModel.cs · `NEOW`=Models.Events/Neow.cs · `RWS`=Rewards/RewardsSet.cs · `CF`=Factories/CardFactory.cs · `PF`=Factories/PotionFactory.cs · `RF`=Factories/RelicFactory.cs ·
`MI`=Entities.Merchant/MerchantInventory.cs · `RSY`=Multiplayer.Game/RestSiteSynchronizer.cs · `CLI`=`referee/vendor/sts2-cli/src/Sts2Headless/RunSimulator.cs` (not under `ref/`).
`R1/R2/R3` = `combat-loop.md`, `rng-decimal.md`, `necrobinder-act1.md`. Single player, ascension 0, Necrobinder, Overgrowth, everything unlocked unless stated. Ascension hooks are listed in §8.4.
`UNVERIFIED` = not confirmed from the decomp. Float note: odds, rarity rolls, shop prices and the unknown-room roll use C# `float` (f32) and `decimal`; these need the same isolated float module as the RNG (decision for the orchestrator, §10).

## 0. Summary
* A run is a fixed pipeline: create player and `RunState` (RNG streams created, no draws) → `InitializeNewRun` (relic bags shuffled on `UpFront`) → `GenerateRooms` (whole run's events and encounter order pre-rolled on `UpFront`) → `EnterAct(0)` (map from its own `act_1_map` rng) → Neow event → map → rooms → boss.
* Almost everything random in Act 1 is decided up front: normal/elite/boss order and event order are lists consumed in order. Only rewards, shops, `?` rolls, Neow options and event outcomes draw during play.
* Streams: run-scoped `RunRngSet` (12 named, R2), per-player `PlayerRngSet` (`rewards`, `shops`, `transformations`), the map rng (`act_N_map`, not in the set), and per-event and per-encounter rngs derived from the seed.
* Rewards, shop prices and `?` rolls are f32/decimal. Card reward pity is a persistent f32 offset.
* Neow gives 3 relic options (2 positive, 1 curse) and its effects reuse the reward machinery (card rewards, relic rewards, transforms), so the reward code comes first.
* The referee drives all of this but only traces combat state, so a run-level dump must be added to the tracer before the run layer can be diff-tested (§9).

## 1. Run init: seed to the first map choice
Real game entry: `NGame.StartNewSingleplayerRun` → `RunState.CreateForNewRun` → `RunManager.SetUpNewSingleplayer` → `StartRun` (`Nodes/NGame.cs:1137-1143,1179-1190`). Order:

| # | Step | Detail | Ref |
|---|---|---|---|
| 0 | Act choice | `act_selection` rng at lobby time picks Act 1 (R3 §4.1); the referee uses `ActModel.GetDefaultList` (always Overgrowth) | `ACT:574-596`, `CLI:280-284` |
| 1 | `Player.CreateForNewRun` | HP/gold from character (66/99), 3 potion slots, `MaxEnergy`, empty `RelicGrabBag`; then `PopulateStartingInventory` | `PL:305-310,334-350` |
| 1a | Starter deck | each `Character.StartingDeck` card `.ToMutable()`, `FloorAddedToDeck=1`, `Deck.AddInternal` in list order (order feeds the first shuffle) | `PL:711-721,731-741` |
| 1b | Starter relic | `ToMutable`, `FloorAddedToDeck=1`, `AddRelicInternal` only: **no `AfterObtained` yet** | `PL:743-752` |
| 1c | Potions | `Character.StartingPotions` (none for Necrobinder) | `PL:346-349`, `CharacterModel.cs:115` |
| 2 | `RunState.CreateForNewRun` | `new RunRngSet(seed)`: 12 `Rng`s created, **no draws**; `RunOddsSet` (UnknownMapPoint rng); `CreateShared` links players, `AddCard` per deck card, shared `RelicGrabBag(refreshAllowed:true)` | `RS:272-286,319-334`, `RRS:104-122` |
| 2a | Player rngs | `InitializeSeed`: `PlayerRngSet(xxh64(seed) + slotIndex)` (slot 0), streams `rewards/shops/transformations`; `PlayerOddsSet` = card-rarity odds (offset `-0.05f`) + potion odds (`0.4f`), **both draw on the player's `rewards` rng** | `PL:328-332`, `PRS:33-47`, `Odds/PlayerOddsSet.cs:22-26` |
| 2b | `AfterCreated` | called on each deck card; empty virtual, no effect on starters | `RS:280-283`, `CardModel.cs:1246` |
| 3 | `SetUpNewSingleplayer` | `InitializeShared`, `InitializeRunLobby`, then `InitializeNewRun`, then `GenerateRooms` | `RM:295-312` |
| 3a | Shared relic bag | `Populate(SharedRelicPool.GetUnlockedRelics, UpFront)`: bucket by rarity in first-seen order (Common/Uncommon/Rare/Shop only), `UnstableShuffle` each bucket on **`UpFront`** | `RM:522`, `RGB:94-114`, `RGB:16-22` |
| 3b | Player relic bag | `Populate(player, UpFront)`: shared pool + `Character.RelicPool` (8 Necrobinder relics), same bucketing and shuffle | `RM:523-526`, `PL:367-373`, `RGB:69-92` |
| 3c | Neow flag | `StartedWithNeow = NeowEpoch revealed` (referee forces true) | `RM:527,579-582`, `CLI:292` |
| 3d | Ascension | `ApplyEffectsTo`: TightBelt `-1` potion slot; AscendersBane card appended silently after the starter deck | `RM:532-535`, `Ascension/AscensionManager.cs:54-66` |
| 4 | `GenerateRooms` | `UpFront` draws in this order: `SharedAncients` shuffle (1 item = Darv, 0 draws), then for acts 2 and 3 `NextInt(list.Count+1)` (2 draws), then per act in order `ActModel.GenerateRooms`; tutorial order mods if `ShouldApplyTutorialModifications` (off in test mode); DoubleBoss only at asc 10 | `RM:743-767`, `UnlockState.cs:85-96`, `ModelDb.cs:189` |
| 5 | `FinalizeStartingRelics` | `AfterObtained` on every starter relic (`BoundPhylactery` has none, so a no-op for Necrobinder) | `RM:725-738`, `NGame.cs:1185` |
| 6 | `EnterAct(0)` | `SetActInternal(0)` (clears visited coords, `UnknownMapPoint.ResetToBase`, `GenerateMap`), then `EnterMapCoord(StartingMapPoint)` → the Neow room | `RM:1321-1350,1356-1370` |

* `ActModel.GenerateRooms` (`ACT:331-386`), all on `UpFront`, in order: (1) events = `AllEvents ++ AllSharedEvents` minus unrevealed Event1/2/3-epoch events, `UnstableShuffle`; (2) 3 weak encounters from a `GrabBag` (weight 1, refilled when empty) via `AddWithoutRepeatingTags`; (3) `15 - 3 = 12` regular encounters likewise; (4) 15 elites likewise; (5) `Boss = NextItem(AllBossEncounters)`; (6) `Ancient = NextItem(unlocked ancients ++ shared subset)` (Overgrowth: only Neow; still one draw).
  `AddWithoutRepeatingTags` (`ACT:419-430`): `GrabAndRemove` with the predicate "no shared tag and not equal to the previous pick"; `GrabIndex` re-draws (`NextDouble() × totalWeight`, f64) until the predicate holds, so draw count varies; if no entry satisfies the predicate it grabs unfiltered (`Helpers/GrabBag.cs:96-124`).
  `AllWeak/AllRegular/AllElite/AllBossEncounters` filter `GenerateAllEncounters()` (alphabetical array, `OG:71-97`) by room type and `IsWeak` (`ACT:149-164`).
  Overgrowth tags (the only ones that matter; others have none): FuzzyWurmCrawlerWeak `[Crawler]`, NibbitsWeak `[Nibbit]`, ShrinkerBeetleWeak `[Shrinker]`, SlimesWeak/SlimesNormal `[Slimes]`, OvergrowthCrawlers `[Shrinker,Crawler]`, FlyconidNormal `[Mushroom,Slimes]`, SlitheringStrangler `[Jaxfruit,Slimes]`, SnappingJaxfruit `[Mushroom,Jaxfruit]` (`Models.Encounters/*.cs`, `EncounterModel.cs:274-281`).
* Streams touched before the first map choice: `UpFront` (3a, 3b, 4), the map rng `act_1_map` (§2), and the Neow event rng (§1.1). Not touched: `Shuffle`, `Niche`, `MonsterAi`, all player streams. (Observed by T3: Neow leaves combat streams alone.)
* Referee differences from the real start: `RunState.CreateForTest` (no `AfterCreated`, default acts), `SetUpTest`, `UnlockState.all` with `NumberOfRuns = 999999999` (so no first-run overrides), `StartedWithNeow` forced true (`RS:304-317`, `CLI:272-312`, `UnlockState.cs:34`).

### 1.1 Neow (`NEOW`, `AEM`)
* Entered like any map point: `RollRoomTypeFor(Ancient)` → `RoomType.Event`; `CreateRoom` uses `Act.PullAncient()` (= `_rooms.Ancient`) (`RM:958,992`). **`MarkRoomVisited(Event)` runs for Neow too, so `eventsVisited` is 1 after Neow** (§7.1) (`RM:1206-1209`).
* `EventModel.BeginEvent` (`EVM:226-259`): `Owner`, then `Rng = new Rng((ulong)((long)runSeed + slotIndex) + xxh64(utf8(Id.Entry)))` (`EVM:234`; `Id.Entry` = `NEOW`), `BeforeEventStarted`, `CalculateVars`, initial options.
  `AEM.BeforeEventStarted` (`AEM:170-191`): for Neow set current HP to 0, then `Heal(maxHp - hp)` (×0.8 at WearyTraveler). Net effect: full heal at run start. Also `Hook.ShouldAllowAncient` can replace the options with a single Proceed (Wax Choker; out of scope) (`AEM:193-207`).
* Option generation (`NEOW:217-286`, no modifiers), all on the event rng, in this order:
  1. `curse = Rng.NextItem(CurseOptions minus IsAllowedAtNeow=false)`. List order (`NEOW:96-108`): CursedPearl, DowsingRod, HeftyTablet, LargeCapsule, LeafyPoultice, NeowsBones, NeowsSacrifice, PrecariousShears, SilkenTress, SilverCrucible. None of these overrides `IsAllowedAtNeow`, and `IsAllowedAtNeow` defaults to `IsAllowed(runState)` (`RelicModel.cs:443-446`); SilverCrucible is single-player only, so all 10 qualify.
  2. Positive list (`NEOW:66-82`, 14): ArcaneScroll, BoomingConch, FishingRod, GoldenPearl, Kaleidoscope, LeadPaperweight, LostCoffer, MassiveScroll, NeowsTorment, NewLeaf, PhialHolster, PreciseScissors, ScrollBoxes, WingedBoots. Remove by curse: CursedPearl→GoldenPearl, HeftyTablet→ArcaneScroll, LeafyPoultice→NewLeaf, PrecariousShears→PreciseScissors, NeowsSacrifice→PhialHolster and LostCoffer.
  3. Unless the curse is LargeCapsule: `Rng.NextBool()` ? append LavaRock : append SmallCapsule. Then `NextBool()` ? NutritiousOyster : StoneHumidifier. Then `NextBool()` ? NeowsTalisman : Pomander (3 draws, or 2 with LargeCapsule).
  4. Remove `IsAllowedAtNeow=false` from the positive list (after the three appends, `NEOW:277-281`): **MassiveScroll** (`IsAllowed` = multiplayer only, so always removed in single player), ScrollBoxes if `CanGenerateBundles` fails (`Relics/ScrollBoxes.cs:23-30,50-55`), Kaleidoscope unless all characters' card pools are unlocked (`Relics/Kaleidoscope.cs:24-31`). WingedBoots/SilverCrucible are single-player only (allowed).
  5. `list2.UnstableShuffle(Rng).Take(2)`, then append the curse: shown options = `[shuffled[0], shuffled[1], curse]`.
* Choosing an option: `RelicCmd.Obtain(relic)` (`AEM:279-287`): history, `AddRelicInternal`, remove from player and shared bags if not stackable, `FloorAddedToDeck = TotalFloor`, then `AfterObtained` (`Commands/RelicCmd.cs:35-55`). Then `Done()` finishes the event; leaving it goes to the map.
* Effects (`Models.Relics/*.cs`, `AfterObtained` unless noted): GoldenPearl +150 gold · CursedPearl Greed curse + 333 gold · NutritiousOyster +11 max HP · NeowsTalisman upgrade last Strike and last Defend of the deck's Basic cards · Pomander upgrade 1 chosen card · PreciseScissors remove 1 chosen card · PrecariousShears remove 2 chosen cards, take 16 damage · NewLeaf transform 1 chosen card on `Rng.Niche` · LeafyPoultice −12 max HP, transform first Strike and first Defend on `PlayerRng.Transformations` · LostCoffer card reward (3, RegularEncounter) + potion reward · SmallCapsule a relic reward · LargeCapsule 2 relics from the bag front + 1 Strike + 1 Defend · HeftyTablet/ArcaneScroll card reward from rares (`Uniform`, `NoUpgradeRoll`) · LeadPaperweight colorless card reward · Kaleidoscope 2 card rewards of 3 cards from other characters' pools (`Rng.Niche` pool shuffle) · ScrollBoxes 2 bundles (2 Common + 1 Uncommon, `PlayerRng.Rewards`), one joins the deck (test mode: bundle 0) · PhialHolster +1 slot, 2 random potions on `CombatPotionGeneration` · NeowsTorment adds NeowsFury · DowsingRod adds Dowsing · NeowsSacrifice Ambergris potion + Guilty · NeowsBones relic rewards shuffled on `PlayerRng.Rewards` + 1 curse on `Rng.Niche` · BoomingConch, FishingRod, WingedBoots, LavaRock (2 extra relics from the Act 1 boss, `TryModifyRewards`), StoneHumidifier, SilkenTress, SilverCrucible are in-run effects.
* The Neow relics are `RelicRarity.Ancient`, so they are never in the grab bags.

## 2. Map generation (Act 1)
Entry: `RunManager.GenerateMap` (`RM:799-831`) → `ActModel.CreateMap` → `StandardActMap.CreateFor` (`SAM:111-114`): `new Rng(runState.Rng.Seed, "act_1_map")` (a plain named `Rng`, **not** a `RunRngSet` stream; seed = hashed run seed, name `act_{CurrentActIndex+1}_map`). Then the Hook `ModifyGeneratedMap`/`AfterMapGenerated` (relics; none at start) and, if `!StartedWithNeow`, the start point becomes a Monster (`RM:820-826`).

Dimensions: width 7, `_mapLength = BaseNumberOfRooms + 1 = 16`, `Grid[7,16]` (rows 0..15). Start point `(3,0)` and boss point `(3,16)` live outside the grid (`SAM:89-95`, `OG:47`). Rows 1..15 are the 15 rooms. Floors: Neow (row 0) + 15 + boss.

Constructor order (`SAM:87-109`), all on the map rng:
1. `GetMapPointTypes(mapRng)` (`OG:134-139`): `rests = NextGaussianInt(7,1,6,7)` (6 or 7), then `unknowns = NextGaussianInt(12,1,10,14)`; elites `round(5 × 1)` = 5 (8 at SwarmingElites 1.6), shops 3 (`MPT:14-33`). `NextGaussianInt` (`Random/Rng.cs:260-281`): loop of two `NextDouble`, Box-Muller with `Math.Sqrt/Log/Sin` (f64), `(int)Math.Round` (banker's), re-draw while outside `[min,max]`. R2 warns the last bit of transcendental results may differ by host.
2. `GenerateMap` (`SAM:207-234`): 7 times: `col = NextInt(0,7)` for a row-1 point (created if missing); on iteration index 1 only, re-draw while the point is already in `startMapPoints`; add to `startMapPoints` (a `HashSet`; later duplicates allowed and just extend the same node); `PathGenerate` from it.
   `PathGenerate` (`SAM:133-143`): until row 15, `GenerateNextCoord`: list `[-1,0,1]` → `StableShuffle(rng)` (sort then Fisher-Yates, R2), try each in order, first one where `!HasInvalidCrossover` wins; target col = clamp(col+d, 0, 6) (so edge moves can repeat the same column); `AddChildPoint`.
   `HasInvalidCrossover(current, targetX)` (`SAM:184-205`): `dx = targetX - col`; false if `dx` is 0 (or 7); else look at the grid point `(targetX, current.row)` in the **current** row and return true if any of its children has `child.col - thatPoint.col == -dx` (an X crossing).
   After 7 paths: every row-15 point gets the boss as child; the start point gets every row-1 point as child.
3. `AssignPointTypes` (`SAM:248-307`): row 15 → RestSite (fixed, `CanBeModified=false`); row `GetRowCount()-7` = 9 → Treasure (Elite when `replaceTreasureWithElites`, false at start); row 1 → Monster (fixed). Queue = `rests × RestSite, 3 × Shop, 5 × Elite, unknowns × Unknown` in that order. `AssignRemainingTypesToRandomPoints` (`SAM:364-385`): up to 3 passes: collect `Unassigned` points in `GetAllMapPoints` order (column-major: col outer, row inner), `StableShuffle` (sorts by coord first), then for each point `GetNextValidPointType`: rotate the queue (dequeue; accept the first type valid at that point, else re-enqueue) and return `Unassigned` if none fit (`SAM:387-403`). Every still-`Unassigned` point becomes Monster; boss = Boss, start = Ancient.
   Validity (`SAM:405-485`): not RestSite/Elite in rows `< 6` (rows 1-5; `IsValidForLower`); no RestSite in rows `>= mapLength-3 = 13` (13, 14); Elite/RestSite/Treasure/Shop may not equal the type of any parent or child; Elite/RestSite/Shop/Monster/Unknown may not equal the type of a sibling (a point that shares a parent). `PointTypesThatIgnoreRules` is empty. (`AssignPointTypesToRandomRows` and `EnsureRowsContainsPointType` are dead code.)
4. `PruneAndRepair` (enabled by default; `MPP:22-32`): 3 times { `PruneDuplicateSegments`; `RepairPrunedPointTypes`, break if it changed nothing }.
   * `FindMatchingSegments` (`MPP:99-108,143-175,249-252`): enumerate every start→boss path by recursive DFS over `Children` (insertion order); for each path, each start index `i` where the point has `>1` child or is row 0, and each end index `i+j` (`j>=2`) with `parents.Count >= 2`: segment key = `row0-endcol,endrow-types` or `col,row-endcol,endrow-types` with the point-type ints (enum order: Unassigned0 Unknown1 Shop2 Treasure3 RestSite4 Monster5 Elite6 Boss7 Ancient8), grouped in a `SortedDictionary<string,…>(Ordinal)`. A group keeps a new segment only if it does not overlap (equal interior point at the same index, for lengths ≥ 3) an existing one. Groups with ≥ 2 segments are the duplicates, in key order.
   * `PrunePaths` (`MPP:254-269`): for each duplicate group in order: `UnstableShuffle(rng)` the group; `PruneAllButLast` (remove nodes of each segment except the last that pass the `PruneSegment` conditions, `MPP:271-317`) and if anything was removed return true; else `BreakAParentChildRelationshipInAnySegment` (remove one edge where a ≥2-child node leads to a node with ≠1 parent) and return true if it broke any. Repeat (`FindMatchingSegments` again) up to 50 times, then throw.
   * `RepairPointType` (`MPP:52-79`): order Shop, Elite, RestSite, Unknown; if the current count is below the target, take Monster points with `CanBeModified`, `StableShuffle`, and convert the ones where `IsValidPointType` holds until the deficit is gone.
   * `RemovePoint` empties the grid cell, detaches all edges. Child/parent sets are `HashSet<MapPoint>` with reference identity (`MapPoint` overrides neither `Equals(object)` nor `GetHashCode`, `MP:30-33`) and are only ever *removed from* after generation, so insertion order (minus removed) is their enumeration order. UNVERIFIED: no later `Add` reuses a freed slot.
5. Post-processing (no rng; `MPO`): `CenterGrid` shifts the whole grid one column when cols 0-1 are empty and cols 5-6 are not (or the reverse) (`MPO:21-81`); `SpreadAdjacentMapPoints` per row repeats until stable: for each node (row order) pick, among columns allowed by its parents/children (each within ±1 and free or itself, iterated ascending, strict `>`), the column maximising the minimum gap to the other nodes in the row (`MPO:197-272`); `StraightenPaths` moves a 1-parent/1-child node toward a neighbouring free column when it is a local extremum (`MPO:86-127`).
   **These shifts change `MapCoord`s**, and the final `(col,row)` is what feeds encounter and monster rngs (§2.1).
6. `State.Map = map`; `RemoveStaleVisitedMapCoords`.

### 2.1 Where the current coord feeds RNG
* `CurrentMapCoord` = last entry of `VisitedMapCoords` (`RS:112-122`), added by `AddVisitedMapCoord` *before* the room is created (`RM:839,863-871`). Neow's coord is `(3,0)`.
* `TotalFloor` = number of `MapPointHistory` entries (`RS:158`); `AppendToMapPointHistory` happens in `EnterMapPointInternal` *before* the room is entered (`RM:921`), so on the first real node `TotalFloor = 2`. (C2's forced `enter_room` has 1.)
* Monster rng: `new Rng(runSeed + col + row + CurrentActIndex + CombatId)` (`Combat/CombatState.cs:245`); encounter rng: `runSeed + TotalFloor + xxh64(Id.Entry)` (C2). Event rng: `runSeed + slot + xxh64(Id.Entry)` (`EVM:234`). `ActFloor = coord.row + 1` (`RM:870,888`).
* `?` rooms and shop blacklist read the *destination's* children and the *previous* history entry (§3).

## 3. Room flow
`EnterMapCoord(coord)` (`RM:833-871`): ignored if already visited → `EnterMapPointInternal(row+1, pointType)` (`RM:876-938`):
1. `ActFloor = row+1`; `ExitCurrentRooms()` (pops and exits every room on the stack, `RM:1139-1162`).
2. `roomType`: `pointType == Unknown` → `UnknownMapPoint.Roll(blacklist)` (below); else the fixed mapping (`RM:976-995`: Shop→Shop, Treasure→Treasure, RestSite→RestSite, Monster→Monster, Elite→Elite, Boss→Boss, Ancient→Event). `TryGetRoomTypeForTutorial` only applies when `NumberOfRuns > 0` is false, i.e. never for the referee (`RM:1000-1026`).
3. `CreateRoom` (`RM:941-966`): Monster/Elite/Boss → `CombatRoom(Act.PullNextEncounter(type).ToMutable())` (list index `visited % count`, `RSET:72-86`); Treasure → `TreasureRoom`; Shop → `MerchantRoom`; Event → `EventRoom(PullNextEvent)` or `PullAncient` for Ancient points; RestSite → `RestSiteRoom`.
4. `AppendToMapPointHistory(pointType, roomType, modelId)`, then `EnterRoom` (`RM:1233-1237`): `ExitCurrentRooms`, `EnterRoomInternal`: `PushRoom`, `Hook.BeforeRoomEntered`, `room.Enter`, and if it is the only room on the stack `Act.MarkRoomVisited(room.RoomType)` (increments the Monster/Elite/Event/Boss visit counter, `RM:1169-1225`, `RSET:88-108`). Note the counter moves *after* `Enter`, and by the **resolved** room type (a `?` that becomes a Monster advances `normalEncountersVisited`).

`UnknownMapPointOdds.Roll(blacklist, runState)` (`UMO:123-177`), state = four f32 values `Monster 0.1, Elite -1, Treasure 0.02, Shop 0.03` (dictionary order Monster, Elite, Treasure, Shop; `UMO:29-47`):
1. If `NumberOfRuns == 0`: first two `?` are Events, the third a Monster (not for the referee).
2. Allowed set = `{Monster, Elite, Treasure, Shop, Event} − blacklist`. `BuildRoomTypeBlacklist` (`RM:660-668`) bans Shop when the previous point's history entry had a Shop room or all of the destination's children are Shops. Default result = Event.
3. `r = rng.NextFloat()` on **`UnknownMapPoint`**; cumulative f32 sum over the allowed, non-negative odds in dictionary order; first `r <= cumulative` wins (Event if none).
4. Update: the rolled type resets to its base; every *other* allowed type in `_baseOdds` adds its base (`Hook.ModifyOddsIncreaseForUnrolledRoomType`, identity by default). Elite's base is `-1`, so it only gets more negative: **elites never appear from `?`**. Event odds are implicit `max(0, 1 - sum of positive odds)`. Reset to base at each act start (`RM:1362`).
First roll: 10% Monster, 2% Treasure, 3% Shop, 85% Event.

| Room | Enter | Exit |
|---|---|---|
| Monster / Elite / Boss | `CombatRoom.EnterInternal` → `StartCombat`: `Encounter.GenerateMonstersWithSlots` (C2), `CreateCreature` per monster, `SetUpCombat` (R1 §1.1), `AfterRoomEntered`, `AfterCombatRoomLoaded` (`Rooms/CombatRoom.cs:119-139,194-228`) | win → `EndCombatInternal` (`Combat/CombatManager.cs:1297-1360`): `ReviveBeforeCombatEnd` (heals a dead player 1), `Hook.AfterCombatEnd`, `room.OnCombatEnded` (gold proportion), `Player.AfterCombatEnd` (clear powers, block, combat state), `Hook.AfterCombatVictory`, then rewards (§4), then proceed to map. HP, deck, relics, potions persist. A loss ends the run |
| Event | §7 | options finish the event; leaving pops the room |
| Shop | §5 | leave |
| RestSite | §6 | leave (remaining options are cleared) |
| Treasure | `TreasureRoom.EnterInternal` starts relic picking (§4.6) | leave |
| Boss (Act 1) | as combat; rewards then `EnterNextAct` → `EnterAct(1)` (`RM:1285-1310`) | end of this spec's scope (§3.1) |

`?` → Monster is a plain monster room from the normal list. Combat-start events (DenseVegetation, etc.) use `EnterCombatWithoutExitingEvent` and return to the event afterwards (`EVM:571-587`, `RM:1255-1283`); not specified here (UNVERIFIED which Overgrowth events use it).

### 3.1 End of Act 1 (RL7)
The Act 1 boss is a normal combat room (`Act.PullNextEncounter(Boss)`, `boss_visited` counts it). Its rewards are gold `NextInt(100, 101)`, the potion roll and a 3-card reward with the boss odds (always Rare), plus `TryModifyRewards` additions (`LavaRock`: 2 relics); no relic of its own (`Rewards/RewardsSet.cs`, `Odds/CardRarityOdds.cs`).
Leaving that rewards screen is the act end: `NRewardsScreen.OnProceedButtonPressed` (terminal, `RoomType.Boss`) → `ActChangeSynchronizer.SetLocalPlayerReady` → `MoveToNextAct` (`ActFloor++`, overwritten below) → `RunManager.EnterNextAct` → `EnterAct(1)`
(`Nodes.Screens/NRewardsScreen.cs:566-590`, `Multiplayer.Game/ActChangeSynchronizer.cs:80-92`, `RM:1285-1355`):
1. `ExitCurrentRooms`; `SetActInternal(1)`: `CurrentActIndex = 1` clears `VisitedMapCoords` and sets `ActFloor = 0` (`RS:69-85`), `UnknownMapPoint.ResetToBase`, `GenerateMap`: Hive's `StandardActMap` from `new Rng(seed, "act_2_map")` (14 rooms, so 15 rows and the boss at row 15; `NextGaussianInt(6, 1, 6, 7)` rests, `StandardRandomUnknownCount - 1` unknowns; `Models.Acts/Hive.cs:47,117-122`), then `ModifyGeneratedMap`/`AfterMapGenerated` (`GoldenCompass`, `FurCoat`; not ported).
2. Act index ≠ 0: `EnterRoomInternal(new MapRoom())` (no `BeforeRoomEntered`, `MapRoom.Enter` does nothing in test mode, `MarkRoomVisited(Map)` counts nothing), then `AfterActEntered` (only the `CursedRun` modifier listens). No heal: the next act's Ancient heals when entered.
3. The last act's boss instead leads to `TheArchitect` event / `WinRun` (not in scope).

**Engine definition.** `RunDecision::ActComplete` is the state right after step 2: `act_index` 1, `act_floor` 0, no visited point, `?` odds at base, `room` `Map`, Hive's map generated, every stream as the game leaves it. The engine stops there (Act 2 is not modeled; `legal_actions` is empty). `MapPointHistory` (so `TotalFloor`) is kept across acts.
The referee does the same: sts2-cli calls `EnterNextAct` itself once the boss's card reward is answered (`HealBetweenActs` disabled), and the tracer prints the new act's `map` and room lists on that line; `pending.next` lists the start point and its children while no point is visited. `tools/diff_run.sh` compares that line (`POLICY=greedy` reaches it).

## 4. Combat rewards
Entry: after `CombatEnded`, `CombatRoom.OfferRoomEndRewards` → `RewardsCmd.GenerateForRoomEnd` → `new RewardsSet(player).WithRewardsFromRoom(room)` then `GenerateWithoutOffering` (`Rooms/CombatRoom.cs:258-271`, `Commands/RewardsCmd.cs:55-60`). The tutorial branch only fires for Ironclad with `NumberOfRuns == 0` (`RWS:258-327`), so it never applies.

`GenerateRewardsFor` (`RWS:206-246`), list order = generation order:

| Room | Rewards (in this order) |
|---|---|
| Monster | Gold(10..20 × proportion, if proportion > 0) → [Potion if `PotionReward.Roll`] → CardReward(3, Regular) |
| Elite | Gold(35..45) → [Potion] → CardReward(3, Elite) → RelicReward |
| Boss | Gold(100) → [Potion] → CardReward(3, Boss) (the last act's boss gives nothing: `RWS:88-91`) |

* Gold amounts: `EncounterModel.MinGoldReward/MaxGoldReward` (Poverty ×0.75 at asc 3, `EncounterModel.cs:64-100`). Monster rooms use `(int)Math.Round((float)min × GoldProportion)`; `GoldProportion = 1f - escaped/spawned` (`EncounterModel.cs:368-371`).
* **RNG order inside a set**: (1) at generation: the potion roll `NextFloat` on the player's `rewards` rng (only here); (2) in `GenerateWithoutOffering` `Populate()` is called in list order, then `Hook.ModifyRewards`, then the list is sorted by `RewardsSetIndex` (Gold 1, Potion 2, Relic 3, Card 5) (`RWS:125-147`). So the draw order is: potion roll, gold `NextInt(min, max+1)`, potion pick, 3 cards, relic.
* **Potion roll** (`PRO:54-72`, state f32 `0.4`): `threshold = odds + eliteBonus × 0.5f` (elite 0.25 → +0.125); `r = rewards.NextFloat()`; `r < threshold` → potion and `odds -= 0.1f`, else `odds += 0.1f`. Boss rolls like Monster. (`ShouldForcePotionReward` hook ignored.)
* **Potion pick** (`PotionReward.Populate` → `PF:45-90`): pool = `Character.PotionPool.GetUnlockedPotions ++ SharedPotionPool` (Necrobinder: 3 character potions, R3); for each potion: `r = rng.NextFloat()`; rarity = Rare if `r <= 0.1f`, Uncommon if `<= 0.35f`, else Common; `NextItem(pool where rarity)`; picked potion removed from the working list. Rng = player `rewards` (`Rewards/PotionReward.cs:54-61`). Claiming into a full belt returns false and the reward stays (`PotionReward.cs:76-93`).
* **Card reward** (`CardReward.Populate` → `CF.CreateForReward`, `CF:89-109,214-261`). Options: pool = `Character.CardPool.GetUnlockedCards` (pool order, epoch-filtered), `Except` already-chosen cards of this reward, then for each of 3 cards:
  1. Rarity: `RollForRarity`: source `Encounter` with RegularEncounter/EliteEncounter/BossEncounter odds types → `CRO.Roll(type)` (changes the offset); any other source → `RollWithBaseOdds`. Then `GetNextAllowedRarity` (wraps Common→Uncommon→Rare→Common if the pool lacks it).
     `RollWithoutChangingFutureOdds(type, offset)` (`CRO:96-111`): `r = NextFloat()` (player `rewards`); `rareThr = baseRare(type) + offset`; Rare if `r < rareThr`; Uncommon if `r < baseUncommon + rareThr`; else Common. Base odds (asc 0): Regular rare 0.03 / uncommon 0.37; Elite rare 0.1 / uncommon 0.4; Boss rare 1.0 (offset forced 0); Shop rare 0.09 / uncommon 0.37. (Scarcity asc 7 changes them: `CRO:13-41`.)
     `Roll` updates the pity offset: Rare → `-0.05f`; otherwise `min(offset + 0.01f, 0.4f)` (`CRO:69-81`). The initial offset is `-0.05f`, so rare needs `r < 0.03 - 0.05 < 0` (never) until the offset reaches ≥ −0.02 after three non-rare rolls. A boss card reward always rolls Rare and resets the offset.
  2. Card: `rng.NextItem(pool.Where(rarity == selected))` on `rewards`. `Uniform` odds skip the rarity roll and pick from all non-Basic non-Ancient cards.
  3. Upgrade roll unless `NoUpgradeRoll`: `RollForUpgrade` (`CF:283-305`): **always** draws `NextFloat()` first (`(decimal)` of the f32 rounds to 7 digits), then if upgradable: odds = `0 + actIndex × 0.25m` for non-rare (0 for rare, `0.125m` scaling at Scarcity); upgrade if `r <= odds`. In Act 1 odds are 0, so it upgrades only on an exact 0.0 draw, but the draw is always consumed.
  4. Then `Hook.TryModifyCardRewardOptions` (relics like Molten Egg; none in scope). `CardRewardAlternative.Generate` adds the skip/other options (UI).
  Picking adds the clone to the deck via `CardPileCmd.Add(card, Deck)`; skipping records history only.
* **Elite relic** (`RelicReward` rarity `None`): `RF.PullNextRelicFromFront(player)` (`RF:26-50`): rarity `RollRarity` = `NextFloat()` on `rewards`: `< 0.5f` Common, `< 0.83f` Uncommon, else Rare; `Player.RelicGrabBag.PullFromFront(rarity, filter, runState)` (first allowed entry of that bucket; empty bucket → refill from original list when `refreshAllowed`, then fall through Shop→Common→Uncommon→Rare; `RGB:116-146,218-243`), fallback `Circlet`; also removed from the shared bag. Claiming goes through `RelicCmd.Obtain`. `Relic.IsAllowed` defaults true (`RelicModel.cs:434`).
* 4.6 **Treasure room** (`TreasureRoom`, `TreasureRoomRelicSynchronizer.cs:89-132`, `Multiplayer.Game/OneOffSynchronizer.cs:128-143`): relic: `RollRarity(TreasureRoomRelics rng)` then `SharedRelicGrabBag.PullFromFront(rarity)` (fallback Circlet); the pick/skip is a player choice and `RelicCmd.Obtain` removes it from both bags. Gold: `rewards.NextInt(42, 53)` (Poverty ×0.75), `(int)` truncation. Extra rewards via `DoExtraRewardsIfNeeded`. First-chest tutorial relic only for `NumberOfRuns == 0`.

## 5. Shop (`MerchantRoom`, `MI`)
`MerchantRoom.EnterInternal` builds `MerchantInventory.CreateForNormalMerchant(player)` (`Rooms/MerchantRoom.cs:52-70`, `MI:78-91`). Draw order (streams: `S` = player `shops`, `R` = player `rewards`):
1. `saleIndex = S.NextInt(5)` (`MI:100`).
2. 5 character cards, types `[Attack, Attack, Skill, Skill, Power]` (`MI:15-22,98-112`). For each: `Populate` → `CF.CreateForMerchant(player, pool.Except(already stocked), type)` (`CF:40-60`): pool minus Basic; `rarity = CardRarityOdds.RollWithoutChangingFutureOdds(Shop)` → **`R.NextFloat()`** (current pity offset, not changed); `GetNextAllowedRarity` over cards of that type; `S.NextItem(cards of rarity & type)`; `RollForUpgrade(base −999999999)` → **`R.NextFloat()`** consumed, never upgrades. Then `CalcCost`: `price = RoundToInt(basePrice × S.NextFloat(0.95f, 1.05f))`, base 50 Common / 75 Uncommon / 150 Rare (`MerchantCardEntry.cs:38-51,118-129`). The entry at `saleIndex` is halved by integer `/2` (`SetOnSale`, which first runs `CalcCost` again: a second price draw). Rounding is half-to-even (confirmed by RL6's diffs).
3. 2 colorless cards, rarities `[Uncommon, Rare]` (`MI:24-28,114-124`): `CF.CreateForMerchant(…, rarity)` (`CF:69-80`): `S.NextItem(pool of rarity)`, `R.NextFloat()` upgrade draw; price base ×1.15 (`RoundToInt(base × 1.15f)`) then ×`S.NextFloat(0.95f, 1.05f)`.
4. 3 relics (`MI:126-139`): rarities are built first: `[RollRarity(R), RollRarity(R), Shop]` (**two `R.NextFloat()` before any pull**); each entry then `RelicFactory.PullNextRelicFromBack(player, rarity, IsAllowedInShops)` (last allowed entry of the bucket, also removed from the shared bag) and `cost = (int)Math.Round(MerchantCost × S.NextFloat(0.85f, 1.15f))`; `MerchantCost` 175 Common, 225 Uncommon, 275 Rare, 200 Shop (`MerchantRelicEntry.cs:37-45`, `RelicModel.cs:304-315`).
5. 3 potions: `PF.CreateRandomPotionsOutOfCombat(player, 3, S)` (rarity roll + item per potion on `S`, no repeats), then per potion `cost = round(base × S.NextFloat(0.95f, 1.05f))` (only when `TestMode.IsOff`; the referee runs in test mode), base 50 Common / 75 Uncommon / 100 Rare (`MerchantPotionEntry.cs:50-71`).
6. Card removal: `75 + 25 × Player.ExtraFields.CardShopRemovalsUsed` (asc 6: 100 + 50×) (`MerchantCardRemovalEntry.cs:20-33`); the card is chosen by the player; the count increments in `OneOffSynchronizer.DoMerchantCardRemoval` after the removal (RL6).
* Buying: `OnTryPurchaseWrapper` (`MerchantEntry.cs:69-96`): out of stock or `Cost > gold` fail; cards `CardPileCmd.Add` to deck then `LoseGold`; relics `LoseGold` then `RelicCmd.Obtain`; potions `TryToProcure` (fail on full belt) then `LoseGold`. After a purchase the slot is cleared, or restocked if `Hook.ShouldRefillMerchantEntry` (default no). `Cost` goes through `Hook.ModifyMerchantPrice` only inside a MerchantRoom (`MerchantEntry.cs:19-30`).
* Leaving a shop has no state change. The `?`→Shop odds are in §3.

## 6. Rest site (`RestSiteRoom`, `RSO`)
* Options (`Entities.RestSite/RestSiteOption.cs:53-74`): `[Heal, Smith]` (+ Mend in multiplayer); then `Hook.ModifyRestSiteOptions` (cards/relics add options, e.g. `ByrdonisEgg` adds Hatch: `Models.Cards/ByrdonisEgg.cs`).
* Heal: `Heal(creature, MaxHp × 0.3m)` (decimal), then `Hook.ModifyRestSiteHealAmount` (relics), `AfterRestSiteHeal`, heal rewards via hook (`HealRestSiteOption.cs:60-113`). `CreatureCmd.Heal` → `HealInternal` → `CurrentHp = (int)Math.Min(CurrentHp + amount, MaxHp)`: **truncation**; 66 max HP heals 19.8, so 40 → 59 (`Commands/CreatureCmd.cs:735-748`, `Creature.cs:478-492`).
* Smith: enabled iff the deck has an upgradable card; the player picks `SmithCount = 1` card, `CardCmd.Upgrade`, `Hook.AfterRestSiteSmith`; cancelling returns false and keeps the options (`SmithRestSiteOption.cs:49-74`).
* After a successful choice the chosen option is removed; unless a hook says otherwise (default: all listeners agree) the **rest of the options are cleared**, so one action per visit; `MiniatureTent` keeps them (`RSY:199-218`, `Models/AbstractModel.cs:2273`, `Hooks/Hook.cs:2262-2272`). No rng is used.

## 7. Events
### 7.1 Event pick (`?` and Event rooms)
* `PullNextEvent` (`ACT:437-443`, `RSET:110-125`): `EnsureNextEventIsValid`: loop at most `events.Count` times: if `events[eventsVisited % n].IsAllowed(runState)` and its id is not in `VisitedEventIds` return, else `eventsVisited++` (no rng). If all fail it logs and allows repeats. Then `Hook.ModifyNextEvent`, `AddVisitedEvent`. After the room is entered `MarkRoomVisited(Event)` does `eventsVisited++` again.
* **Neow's room already counted once**, so the first `?` event is looked up at index 1 (unless a skip moved it), `eventsVisited` after it is ≥ 2.
* The list is `UnstableShuffle`d `AllEvents(13) ++ AllSharedEvents(18)` on `UpFront` (§1), epoch-gated removal first (`ACT:334-347`). `IsAllowed` in Act 1 (index 0; all others default true: AromaOfChaos, SapphireSeed, SunkenStatue, TabletOfTruth, Wellspring, SelfHelpBook, ThisOrThat):
  ByrdonisNest no event pet (relic `AddsPet` or `ByrdonisEgg` in deck) · DenseVegetation, JungleMaze true in single player · LuminousChoir gold ≥ canonical `Gold` var and relic bag has a relic · MorphicGrove gold ≥ 100 and ≥ 2 transformable cards · UnrestSite HP ≤ 70% of max · WhisperingHollow gold ≥ 44 · WoodCarvings a removable Basic card · BrainLeech, RoomFullOfCheese act < 2 · SlipperyBridge `TotalFloor > 6` and a removable card · TheLegendsWereTrue act 0, HP ≥ 10 · act ≥ 1 only (so excluded): CrystalSphere, DollRoom, FakeMerchant, PotionCourier, RanwidTheElder, RelicTrader, StoneOfAllTime, Symbiote, WelcomeToWongos · TeaMaster act < 2 and gold ≥ 150 · TheFutureOfPotions ≥ 2 potions · WarHistorianRepy never (`Models.Events/*.cs`, `ModelDb.cs:157-180`). SelfHelpBook/ThisOrThat/AromaOfChaos/Sunken/Tablet/Wellspring/Sapphire have no override (default true).
  Reading `IsAllowed` happens on the canonical instance, so `DynamicVars` bases are the canonical ones (LuminousChoir, UNVERIFIED value).
### 7.2 Shape of an event
* `EventRoom.EnterInternal` → `EventSynchronizer.BeginEvent`: the canonical event is `ToMutable()`d and `BeginEvent` runs (§1.1: `Owner`, event `Rng`, `BeforeEventStarted`, `CalculateVars`, `GenerateInitialOptions` → `SetEventState(description, options)`) (`Rooms/EventRoom.cs:56-88`, `EVM:226-269,526-545`).
* An option is `EventOption(model, onChosen, textKey, hoverTips…)`; `Chosen()` runs `onChosen` once (`DisableOnChosen`) (`Events/EventOption.cs:174-185`). The callback mutates the run (gold, HP, deck, relics, rewards) and then either `SetEventState(newDesc, newOptions)` for the next page or `SetEventFinished(desc)` (empty options → `IsFinished`, then a Proceed leaves the room) (`EVM:478-483`).
* RNG sources inside events: the event `Rng` (`CalculateVars`, e.g. WhisperingHollow `Gold += Rng.NextInt(-9, 10)`, LuminousChoir `Gold -= Rng.NextInt(0,50)`), or run/player streams for gameplay content (`Wellspring` below uses the player's `rewards`). Choices that need the player (pick a card) go through `CardSelectCmd` (referee: `select_cards`).
* A canonical `EventModel` is stateless; vars are on the clone. `OnEventFinished` runs once through `EnsureCleanup`.
### 7.3 Example A: `ByrdonisNest` (`Models.Events/ByrdonisNest.cs`)
* Vars: `MaxHp 7`. `IsAllowed`: player has no event pet. No rng.
* Options: **Eat** → `GainMaxHp(+7)` (raises max HP and heals the same amount, `CreatureCmd.cs:838-851`), finish. **Take** → create `ByrdonisEgg` (Quest rarity, Unplayable, `MaxUpgradeLevel 0`, adds the Hatch rest-site option), `CardPileCmd.Add(Deck)`, finish. Both finish on the first page; Proceed leaves.
### 7.4 Example B: `Wellspring` (`Models.Events/Wellspring.cs`)
* Vars: `BatheCurses 1`. Always allowed.
* **Bottle** → `item = player.PlayerRng.Rewards.NextItem(CharacterPotionPool.GetUnlockedPotions ++ SharedPotionPool.GetUnlockedPotions)` (a single uniform draw over all potions, **not** the rarity-weighted `PotionFactory`), then `RewardsCmd.OfferCustom([PotionReward(item)])` (claim blocked by a full belt), finish.
* **Bathe** → `CardSelectCmd.FromDeckForRemoval(1)` → `RemoveFromDeck`, then `AddCursesToDeck([Guilty] × 1)`, finish.
* Takeaway: events mix their own rng, shared streams and the reward/select machinery; porting the machinery first makes each event small.

## 8. State carried between combats
### 8.1 Player (`Player.ToSerializable`, `PL:375-401`)
Character id, current HP, max HP, max energy, max potion slots (3; Phial Holster +1; TightBelt −1), gold, `PlayerRngSet` (3 rng states), `PlayerOddsSet` (card-rarity offset f32, potion odds f32), player `RelicGrabBag` (per-rarity ordered lists + the original list for refresh), deck (ordered cards: id, upgrade level, enchantment, `FloorAddedToDeck`, saved properties), relics (ordered, with `[SavedProperty]` counters such as `WingedBoots.TimesUsed`), potion slots (nullable, slot-indexed), `ExtraPlayerFields` (e.g. `CardShopRemovalsUsed`), `UnlockState`, discovered cards/relics/potions (cosmetic).
### 8.2 Run (`RunManager.ToSave`, `RM:670-709`)
Acts (each `RoomSet`: shuffled event list, normal/elite lists, boss, ancient, `eventsVisited`, `normalEncountersVisited`, `eliteEncountersVisited`, `bossEncountersVisited`, `RSET:127-142`), `CurrentActIndex`, the **map** (full topology and types, since a loaded run reuses the saved map), `VisitedMapCoords`, `MapPointHistory`, `EventsSeen` (`VisitedEventIds`), `RunRngSet` (all 12 streams: seed + counter, R2), `RunOddsSet` (the four unknown-room f32 odds), `SharedRelicGrabBag`, `ExtraFields.StartedWithNeow`, ascension, modifiers.
### 8.3 Not persisted
Combat state (piles, energy, powers, block; `Player.AfterCombatEnd` clears them, `PL:837-842`), monster state, shuffle order of a past combat. The combat rngs (`Shuffle`, `MonsterAi`, `CombatCardGeneration`, …) do persist as run-scoped streams (R1), so every combat's draws depend on all earlier combats.
### 8.4 Ascension 0–10 (RL8)
Every `AscensionLevel` / `AscensionHelper` / `HasAscension` check outside `Models.Monsters` and the UI (paths under `ref/decomp/MegaCrit.Sts2.Core.`). `HasLevel(l)` is `level >= l`; `RunManager.HasAscension` is false while no run is in progress. Ported in `run/ascension.rs` and the files named; `Run::new` accepts 0–10 (`AscensionManager.maxAscensionAllowed`).

| Level | Effect | Decomp ref | Status |
|---|---|---|---|
| 1 SwarmingElites | `NumOfElites` = `(int)Math.Round(5f × 1.6f)` = 8 (every act map) | `Map/MapPointTypeCounts.cs:14` | ported (`map.rs`) |
| 2 WearyTraveler | Neow heals the missing HP × 0.8m (66 → 52) | `Models/AncientEventModel.cs:180` | ported (`neow.rs`) |
| 3 Poverty | combat gold min/max (`double` × 0.75, `(int)`): monster 7–15, elite 26–33, boss 75; treasure `NextInt(42, 53)` × 0.75 truncated | `Models/EncounterModel.cs:64-99`, `Multiplayer.Game/OneOffSynchronizer.cs:129-143` | ported (`rewards.rs`, `mod.rs`) |
| 4 TightBelt | `SubtractFromMaxPotionCount(1)`: 2 starting slots | `Entities.Ascension/AscensionManager.cs:56` | ported (`mod.rs`) |
| 5 AscendersBane | `AscendersBane` joins the starting deck (floor 1, silent) | `AscensionManager.cs:60` | ported (`mod.rs`; combat setup also adds it for A5+ loadouts) |
| 6 Inflation | card removal 100 + 50 × removals (else 75 + 25 ×) | `Entities.Merchant/MerchantCardRemovalEntry.cs:20-33` | ported (`shop.rs`) |
| 7 Scarcity | rare odds regular 0.0149f, elite 0.05f, shop 0.045f (else 0.03, 0.1, 0.09); `RarityGrowth` 0.005f (else 0.01); `regularCommonOdds` 0.615f is never read by a roll; card-reward upgrade odds scale 0.125m per act index (else 0.25m), 0 in Act 1 | `Odds/CardRarityOdds.cs:13-41`, `Factories/CardFactory.cs:23,297` | ported (`odds.rs`; upgrade scale has no Act 1 effect) |
| 8 ToughEnemies, 9 DeadlyEnemies | monster HP / damage / moves | `Models.Monsters/*` | combat (already ported) |
| 10 DoubleBoss | after the last act's `GenerateRooms`, `UpFront.NextItem` of its other bosses is `SecondBoss` | `Runs/RunManager.cs:761-765` | ported (`rooms.rs`: the draw; the fight is Act 3, not reached) |

Not ascension-dependent (checked): rest heal, card/relic/potion shop prices, potion reward odds (`RewardsSet.RollForPotionAndAddTo` reads `AscensionManager` but never uses it), events, Neow's options, starting gold and HP.

## 9. Referee coverage (`referee/README.md`, `CLI`)
| Part | Can the referee drive it? | Can it trace it? |
|---|---|---|
| Run init, Neow | Yes: `start_run`, `choose_option {option_index}`; `select_cards` for card choices (`CLI:265-325,2136`). Seed `T3S0` etc. | No: `dump_state` outside combat returns only `seed, act, floor, first_encounters` (`tracer/TraceDump.cs:19-32`). The sts2-cli decision JSON shows event text/options but not rng state |
| Map | Yes: `get_map` returns all nodes, types, children, visited/current (`CLI:5134-5175`); `select_map_node {col,row}` calls the real `EnterMapCoord` (`CLI:956-985`) | Map yes via `get_map` (boss node appended after the grid rows); good first differential target, needs no play |
| Room order | `first_encounters` gives the first 3 normals; events/elite/boss order not exposed (reflection on `_rooms` would give all) | partial |
| Combat rewards | Gold, potion and relic rewards are auto-claimed, card reward is presented (`select_card_reward`, `skip_card_reward`) after `RewardsSet.GenerateWithoutOffering` (`CLI:3395-3472`); rewards are the real generator | Card reward options are in the decision JSON; gold/potion/relic only show in `player` state afterwards. No odds or rng dump |
| Treasure | Auto-picks relic 0 and takes gold (`CLI:3817-3890`); the pick/skip choice is not drivable | final state only |
| Shop | `buy_card/relic/potion {index}`, `remove_card` (`CLI:1808-1930,3807`); inventory and prices are in the decision JSON | prices yes, rng no |
| Rest | `choose_option` (Smith via `select_cards`) (`CLI:2136-2230`) | HP/deck only |
| Events | `choose_option`, `select_cards`; `enter_room {type: event, event: ID}` forces one (`Program.cs:200-207`) | decision JSON only |
| Act transition | `proceed` after the boss calls the real `EnterNextAct` then `HealBetweenActs`, a **sts2-cli invention** (heal to full, ×0.8 at asc ≥ 2) that does not exist in the game (the next act's Ancient event heals via `AEM:170-191`); `setup.sh` disables the heal, so the transition is an oracle (§3.1) | yes: the first Act 2 line (map, rooms, odds, streams) |
* Missing from the tracer (needed before diff testing the run layer): a `dump_run` command that reads, read-only, (a) `RunRngSet` and `PlayerRngSet` states and counters (private `_rngs` via reflection), (b) `PlayerOddsSet` and `RunOddsSet` values, (c) both relic grab bags, (d) the `RoomSet` lists and counters, (e) the full map, (f) deck/relics/potions/HP/gold, (g) event id, option list and `eventsVisited`, (h) the pending `RewardsSet` and shop inventory with prices.
* Known harness limits: one run per process; `UnlockState.all` (no epoch gating, no first-run order, no act_selection); starting deck made by `CreateForTest` skips `AfterCreated` (no effect for starters); the referee cannot choose treasure/reward order; the `proceed`/`HealBetweenActs` act heal above.

## 10. Recommended porting order and rough size (Rust lines incl. tests, ±50%)
1. **Run state + init (§1)**: `RunState`, stream set (reuse R2), player odds, `RelicGrabBag`, `GenerateRooms` (events, encounter bags, boss, ancient), act tags. ~700. Needs the shared relic pool and Necrobinder pool in game order (data port) and the Overgrowth event/encounter lists. Test: `first_encounters` and the new `dump_run` (rooms, bags).
2. **Map (§2)**: grid, path gen, point types, pruning, post-processing. ~900. Test: `get_map` for many seeds; this is the highest-risk single algorithm (HashSet orders, SortedDictionary keys, f64 Gaussian). Do it early because coords feed combat rngs.
3. **Room flow + unknown roll + encounter queue (§3)**: ~400. Hook into the existing combat entry (C2 already does encounters).
4. **Rewards (§4)**: odds (f32), `GrabBag`-free; card/potion/relic/gold rewards, treasure. ~600. Needs the full Necrobinder card pool order, potion pools and relic buckets as data. Test: forced fights + card reward decision JSON.
5. **Rest site + treasure (§6, §4.6)**: ~200.
6. **Shop (§5)**: ~450 (price float rounding is the risk).
7. **Event framework (§7.2) + 2 example events + `?` resolution**: ~400, then each remaining Overgrowth/shared event 40-150 each (13 + 8 conditionally allowed shared).
8. **Neow (§1.1)**: option generation ~150, relic effects ~600, depends on 4 and the transform/select commands.
9. **Boss + end of Act 1 (and `LavaRock`-style hooks)**: ~150. Done in RL7 (§3.1).
10. **Tracer `dump_run` (C#)**: ~300 lines, do it with step 1.
* Decisions for the orchestrator: (a) allow f32/decimal in a `run_odds` module next to the RNG float exception; (b) make `UnlockState` (epochs, `NumberOfRuns`, discovered acts/encounters) an explicit config, defaulting to the referee's all-unlocked veteran state; (c) pool orders (cards, potions, relics) must come from `GenerateAllCards/Potions/Relics` array order; the content-api spec should expose them.
* UNVERIFIED summary: `Mathf.RoundToInt` mode; potion pool enumeration order for `NextItem`; behaviour of `NextItem` on an empty rarity-filtered potion list; HashSet slot reuse after removals; where `CardShopRemovalsUsed` increments; Kaleidoscope, ScrollBoxes, MassiveScroll effects; which Overgrowth events start combats; relic-specific `Hook.*` modifiers on rewards/shops/map; the real `StartRunLobby` act selection at a real profile; `ModifyUnknownMapPointRoomTypes` implementers.
