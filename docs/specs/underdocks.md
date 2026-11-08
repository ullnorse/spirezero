# Underdocks: the alternate Act 1 (S-UD)

Research only, no engine code. Decomp paths are under `ref/decomp/MegaCrit.Sts2.Core.*`; `UD` = `Models.Acts/Underdocks.cs`,
`OG` = `Models.Acts/Overgrowth.cs`, `ACT` = `Models/ActModel.cs`. Selection details are also in `necrobinder-act1.md` §4.1.

## 1. How Act 1 becomes Overgrowth or Underdocks
* `StartRunLobby.BeginRunLocally` (`Multiplayer.Game.Lobby/StartRunLobby.cs:470-473`): `rng = new Rng(GetDeterministicHashCode(seed), "act_selection")`
  (the `Rng(ulong seed, string name)` ctor, `Random/Rng.cs:54`, i.e. `hash(seed) + hash("act_selection")`, a stream of its own, not part of `RunRngSet`);
  `list = ActModel.GetRandomList(rng, unlockState, isMultiplayer)`; `list[0] = GetAct(Act1) ?? list[0]`. `Act1` is the lobby setting, default `"random"`
  (`:107`); `"overgrowth"` / `"underdocks"` force it (`:503-514`), anything else leaves the roll.
* `ModelDb.Acts` = Overgrowth, Underdocks, Hive, Glory (`ModelDb.cs:299-321`); `ActsByIndex` buckets by `Index` (`:323-345`): index 0 = `[Overgrowth, Underdocks]`
  (`OG:49`, `UD:44`), 1 = `[Hive]`, 2 = `[Glory]`.
* `GetRandomList` (`ACT:538-565`), per index: keep the acts with `IsUnlocked`; if one is non-default, singleplayer, **test mode off** and absent from
  `SaveManager.Progress.DiscoveredActs` (`ProgressState.cs:91`), it is returned at once with no draw (the first run after the unlock always shows it);
  else `rng.NextItem(candidates)` = `NextInt(0, n)`, drawn even for one candidate (acts 2 and 3). So with everything unlocked and discovered, Act 1 is
  `NextInt(0, 2)` on the first draw of the stream: 0 Overgrowth, 1 Underdocks. A `RandomCharacter` then draws on the same stream (`:475-479`).
* Gating: `Overgrowth.IsUnlocked` is true, `IsDefault` true (`OG`); `Underdocks.IsUnlocked = unlockState.IsEpochRevealed<UnderdocksEpoch>()` (`UD:107-110`),
  `IsDefault` false (`UD:46`). `UnderdocksEpoch` (`Timeline.Epochs/UnderdocksEpoch.cs`) reveals `Colorless2`, `Relic2`, `Potion2` epochs. Nothing else in the run
  depends on which Act 1 it is: no `Act is Underdocks` or `Id ==` check exists in events, relics, cards, powers, monsters or rewards (grepped).
* Test mode (`TestMode.IsOn`): the forced-first-sighting branch is skipped (`ACT:550`), so the draw is always made. The referee never reaches
  `GetRandomList`: `RunSimulator.StartRun` (`referee/vendor/sts2-cli/src/Sts2Headless/RunSimulator.cs:280`) calls `RunState.CreateForTest(players, ascensionLevel, seed)`
  without `acts:`, and `CreateForTest` takes `acts ?? ActModel.GetDefaultList()` (`Runs/RunState.cs:311`; default = the `IsDefault` act per index:
  Overgrowth, Hive, Glory, `ACT:567-590`). See `test-mode.md`, last row of the gameplay table.
* What the engine does today (`crates/sts2-core/src/run`): nothing. `rooms.rs:237` `DEFAULT_ACTS = [OVERGROWTH, HIVE, GLORY]` is the only act list;
  `generate_rooms` (`rooms.rs:322`) and `Run::new` (`mod.rs:404-408`) use it; there is no `act_selection` stream, no `UNDERDOCKS` `ActInfo`,
  `room_type_of_encounter` (`rooms.rs:59`) searches `DEFAULT_ACTS` only, and `ActMap::generate(rngs, act_index, ..)` (`map.rs:140`) picks the shape by index.
* Decision needed for the product: the engine models an experienced profile (`test-mode.md` #4/#5), i.e. everything unlocked and discovered. Then the real
  game's Act 1 for seed S is `Rng::named(hash(S), "act_selection").next_int(2)` (`hash` = `GetDeterministicHashCode`); the referee cannot check this draw (see §5), so it needs a unit
  test written from the decomp, like the forced first sighting.

## 2. `ActModel` parameters (Underdocks vs Overgrowth)
| Parameter | Underdocks | Overgrowth | Ref |
|---|---|---|---|
| `Index`, `IsDefault` | 0, false | 0, true | `UD:44,46`, `OG:49,51` |
| `BaseNumberOfRooms` | 15 | 15 | `UD:42`, `OG:47` |
| `NumberOfWeakEncounters` | 3 | 3 | `UD:40`, `OG:45` |
| `GetMapPointTypes` | `NextGaussianInt(7,1,6,7)` rests, then `StandardRandomUnknownCount` | identical | `UD:112-117`, `OG:134-139` |
| `CreateMap` | `StandardActMap.CreateFor` (not overridden) | same | `ACT:522`, `Map/StandardActMap.cs:111-113` |
| Ancients | `Neow` only (dropped if `NeowEpoch` unrevealed) | same | `UD:22,95-101` |
| Boss discovery order | WaterfallGiant, SoulFysh, LagavulinMatriarch | Vantom, CeremonialBeast, TheKin | `UD:15` |
| `ApplyActDiscoveryOrderModifications` | empty | fixed first-run order if `NumberOfRuns == 0` | `UD:103`, `OG:110-127` |
| Events (own) | 10 (below) + the 18 shared = 28 | 13 + 18 = 31 | `UD:26`, `OG:23` |
| Encounters | 20: 4 weak, 10 regular, 3 elite, 3 boss | 22 | `UD:66-91` |
| Act-specific relics, potions, powers | none | none | grep |
Both discovery-order hooks are off in test mode and, for a seen profile, in the real game (`test-mode.md` #4).

**Room generation** (`ACT:331-386`, `RunManager.GenerateRooms` `:743-767`) is the same code with different lists; the engine's `generate_act_rooms` handles it once
`UNDERDOCKS` exists. The draw count differs from Overgrowth because the lists do: the event shuffle is over 28 not 31 events, the weak bag has 4 entries, the regular bag 10,
the elite bag 3 (refilled every 3 of the 15 picks), the boss is `NextItem` of 3. `AddWithoutRepeatingTags` uses `EncounterModel.Tags` (`EncounterModel.cs:109,274`):
`Slugs` on both CorpseSlugs encounters, `Seapunk` on both Seapunk ones; no other tags.

**Events** (the act's own, `UD:26-37`): AbyssalBaths, DrowningBeacon, EndlessConveyor, PunchOff, SpiralingWhirlpool, SunkenStatue (shared with Overgrowth,
ported), SunkenTreasury, DoorsOfLightAndDark, TrashHeap, WaterloggedScriptorium. `Event1/2/3Epoch` gating only removes ColorfulPhilosophers, Reflections and
the like (`Timeline.Epochs/Event[123]Epoch.cs`), none of them Underdocks events.

## 3. Encounters and monsters (`Models.Encounters/*.cs`, `Models.Monsters/*.cs`)
HP is `A0 / with `ToughEnemies`` from `MinInitialHp` (== `MaxInitialHp` unless two numbers are given; ranges are rolled). Ids are the referee's (SCREAMING_SNAKE).
| Tier | Encounter | Monsters (slot order) | HP A0 → `ToughEnemies` | New powers or mechanics |
|---|---|---|---|---|
| weak | CorpseSlugsWeak (tag Slugs) | CorpseSlug ×2 | 25-27 → 27-29 | `Ravenous`, Frail; `EnsureCorpseSlugsStartWithDifferentMoves(rng)` |
| weak | SeapunkWeak (tag Seapunk) | Seapunk | 44-46 → 47-49 | Strength |
| weak | SludgeSpinnerWeak | SludgeSpinner | 37-39 → 41-42 | Strength, Weak |
| weak | ToadpolesWeak | Toadpole ×2 (`IsFront` true, false) | 21-25 → 22-26 | Thorns |
| normal | CorpseSlugsNormal (Slugs) | CorpseSlug ×3 | as above | as above |
| normal | CultistsNormal | CalcifiedCultist, DampCultist | 38-41 / 51-53 | Ritual |
| normal | FossilStalkerNormal | FossilStalker | 51-53 → 54-56 | Frail, `SuckPower` |
| normal | GremlinMercNormal | GremlinMerc (+ FatGremlin, SneakyGremlin, summoned) | 47-49 → 51-53 | `Thievery`, `Surprise`, Strength, Weak; **Escape** (FatGremlin flees; `EscapedCreatures`, `CalculateGoldProportion` 1 / 0.5 / 0) |
| normal | HauntedShipNormal | HauntedShip | 63 → 67 | Dazed into the combat piles, Weak |
| normal | LivingFogNormal | LivingFog (slot `livingFog`), spawns GasBomb | 80 → 82 | `Smoggy` power, `Smog` affliction, `GasBomb` (Minion, 7 → 8 HP), SummonIntent |
| normal | PunchConstructNormal | PunchConstruct | 55 → 60 | Artifact, Frail |
| normal | SeapunkNormal (Seapunk) | CalcifiedCultist, Seapunk | 38-41 / 44-46 | |
| normal | SewerClamNormal | SewerClam | 56 → 58 | Plating, Strength |
| normal | TwoTailedRatsNormal | TwoTailedRat ×3, `StarterMoveIndex = Rng.NextInt(3)` rotated | 17-21 → 18-22 | Frail, summon (`CALL_FOR_BACKUP_MOVE`), encounter `Rng` seeded `Seed + TotalFloor + hash(Id)` (`EncounterModel.cs:263`) |
| elite | PhantasmalGardenersElite | PhantasmalGardener ×4 (slots first..fourth) | 26-31 → 27-32 | `Skittish`, Strength |
| elite | SkulkingColonyElite | SkulkingColony | 75 → 80 | `HardenedShell`, Strength |
| elite | TerrorEelElite | TerrorEel | 140 → 150 | `Shriek` (70), Vigor, Vulnerable |
| boss | WaterfallGiantBoss | WaterfallGiant (378-line AI) | 240 → 250 | `SteamEruption` (18), Weak |
| boss | SoulFyshBoss | SoulFysh | 211 → 221 | Intangible, Vulnerable, `Beckon` status card |
| boss | LagavulinMatriarchBoss | LagavulinMatriarch (wakes at ≤ half HP, `IsShellAwake`) | 222 → 233 | `Asleep`, Plating 12, Strength, Dexterity |
Event encounter (not in `GenerateAllEncounters`): `PunchOffEventEncounter` = 2 PunchConstruct (HP 47/55 and 52/55 on entry), started by `PunchOff`.
Totals: **22 monsters** (CalcifiedCultist is shared), **20 act encounters + 1 event encounter**; none ported (`content/monsters` has no Underdocks monster).

## 4. Map: Underdocks vs Overgrowth
Identical. Neither act overrides `CreateMap`; `StandardActMap.CreateFor` builds from `new Rng(seed, "act_<CurrentActIndex+1>_map")` with `BaseNumberOfRooms` 15
and the same `GetMapPointTypes`, and reads `runState.Act` only for those two values. For Act 1 the map is therefore the same function of the seed in both acts, and the engine's
`act_shape(0)` (`map.rs:220`) already serves it. Only the `OG`-specific data (colours, music, backgrounds) differ, all cosmetic. Rooms differ only through the encounter and event
lists above.

## 5. Referee: forcing Underdocks
* **Forced-encounter mode already works.** `enter_room` looks the encounter up by id in `ModelDb` (`RunSimulator.cs:454`), so the run's act is irrelevant. Run on main's build
  (`STS2_REFEREE=referee/builds/current`, `referee/run.sh RL1S0 x.json`, Necrobinder A0, two `end_turn`s): all 20 act encounters
  (`CORPSE_SLUGS_WEAK/NORMAL`, `SEAPUNK_WEAK/NORMAL`, `SLUDGE_SPINNER_WEAK`, `TOADPOLES_WEAK`, `CULTISTS_NORMAL`, `FOSSIL_STALKER_NORMAL`, `GREMLIN_MERC_NORMAL`,
  `HAUNTED_SHIP_NORMAL`, `LIVING_FOG_NORMAL`, `PUNCH_CONSTRUCT_NORMAL`, `SEWER_CLAM_NORMAL`, `TWO_TAILED_RATS_NORMAL`, `PHANTASMAL_GARDENERS_ELITE`, `SKULKING_COLONY_ELITE`,
  `TERROR_EEL_ELITE`, `SOUL_FYSH_BOSS`, `WATERFALL_GIANT_BOSS`, `LAGAVULIN_MATRIARCH_BOSS`) and `PUNCH_OFF_EVENT_ENCOUNTER` start and trace (3 lines, rc 0), with HP, powers and
  first moves printed. Caveat: those fights run inside an Overgrowth run (rooms, map and reward context are Overgrowth's); combat state does not depend on it.
  Forced events: `enter_room` type `event` takes any `EVENT` id (`RunSimulator.cs:462-470`), but `referee.py` only sends `combat` (`referee.py:~110`), so event forcing is a harness gap.
* **Run mode cannot choose the act.** `StartRun` never passes `acts:`. Nothing in `referee/tracer` or `setup.sh` touches it (`RunDump.cs:33-34` only reports `act_id`).
  A run-mode diff of Underdocks (map, room lists, natural fights and events, shop/rest context) needs the act forced.
* **Smallest change** (≈3 lines, untested: `setup.sh` was not run): in `referee/setup.sh`, next to the existing `sed -i` patches of `RunSimulator.cs`, add
  `sed -i 's|^\( *\)players: new\[\] { player },|&\n\1acts: Environment.GetEnvironmentVariable("STS2_ACT1") == "underdocks" ? new MegaCrit.Sts2.Core.Models.ActModel[] { ModelDb.Act<MegaCrit.Sts2.Core.Models.Acts.Underdocks>(), ModelDb.Act<MegaCrit.Sts2.Core.Models.Acts.Hive>(), ModelDb.Act<MegaCrit.Sts2.Core.Models.Acts.Glory>() } : null,|' "$rs"`
  (`CreateForTest` already does `.Select(a => a.ToMutable())`, and a `null` falls back to the default list). In `referee.py` `Game.__init__`, set `env["STS2_ACT1"] = spec["act1"]` when the
  actions file has `"act1": "underdocks"` (`Game` is constructed per run; the field needs passing in). No new protocol command and no change to `tracer/*.cs`; the build hash changes
  (it hashes `setup.sh`), so it is a new immutable build. `GenerateRooms` and `EnterAct(0)` then use Underdocks (they read `State.Acts`).
* The referee cannot test the `act_selection` roll (it skips `GetRandomList`); that stays a decomp-only port with a unit test, like the forced first sighting.

## 6. Porting table
| Area | Count | Status / gap |
|---|---|---|
| Act data | 1 `ActInfo` (+ shared lists) | Missing. `UNDERDOCKS` in `rooms.rs`: 20 encounters with tags, 10 events, `rooms` 15, `weak_encounters` 3, ancient `NEOW`. `room_type_of_encounter`, `EVENT_ENCOUNTERS` (+ `PUNCH_OFF_EVENT_ENCOUNTER`) must look beyond `DEFAULT_ACTS` |
| Act selection | 1 stream | Missing: `act_selection` draw in `Run::new`, `acts` list in `RunState`, an `act1` override (loadout / run config), `generate_rooms(acts)` |
| Map | 0 | None: `act_shape(0)` is reused |
| Monsters | 22 (21 new + the shared cultist pair counted once) | All unported. Needs `MonsterKind` + AI each (WaterfallGiant 378 lines, LagavulinMatriarch 286, SoulFysh 243, TwoTailedRat 241 are the big ones) |
| Encounters | 20 + 1 event | All unported; TwoTailedRats and CorpseSlugs draw from the encounter `Rng` in `generate_monsters` (the trait already gets `rng`, `encounters/mod.rs:188`) |
| Powers | 11 missing | Ravenous, Suck, Thievery, Surprise, Smoggy, Skittish, HardenedShell, Shriek, SteamEruption, Asleep, Intangible. Present: Thorns, Plating, Ritual, Frail, Minion, Strength, Weak, Vulnerable, Artifact, Vigor |
| Cards, afflictions | 2 | `Beckon` (status card) and the `Smog` affliction (SmoggyPower afflicts every Skill in all piles on play and on entry). `Dazed` exists |
| Engine mechanics | 3 | `CreatureCmd.Escape` + `EscapedCreatures` + `CalculateGoldProportion` (`rewards.rs:41` assumes 1); mid-combat summon of GasBomb, rats and gremlins (the Phrog wrigglers already add creatures); hp-threshold wake-up (`AsleepPower`, Matriarch) |
| Events | 9 new (+ SunkenStatue done) | AbyssalBaths, DoorsOfLightAndDark, DrowningBeacon, EndlessConveyor, PunchOff, SpiralingWhirlpool, SunkenTreasury, TrashHeap, WaterloggedScriptorium. `IsAllowed` rules to add in `events.rs:~185`: PunchOff `TotalFloor ≥ 6`, EndlessConveyor all gold ≥ 120, Scriptorium gold ≥ 55, TrashHeap HP > 5, Whirlpool a deck card the `Spiral` enchantment fits |
| Event dependencies | ~20 | Relics FresnelLens, DarkstonePeriapt, DreamCatcher, HandDrill, MawBank, TheBoot; GlowwaterPotion; enchantments Spiral, Steady (the engine has 10 others); cards Caltrops, Clash, Distraction, DualWield, Entrench, FeedingFrenzy, HelloWorld, Outmaneuver, Rebound, RipAndTear, Stack (all `EventCardPool`); curses Greed and Injury exist. EndlessConveyor also needs `TransformToRandom`, `OfferCustom`, `Rng.Rewards` potions |
| Referee | 1 sed + 1 env key | Forced encounters already OK; run mode needs the §5 change; event forcing needs `referee.py` to send `enter_room` type `event` |
| Regression | – | `regress`/`final_check` need the act as a matrix axis (`act1`), 20 encounters × characters, plus run-mode seeds with `"act1": "underdocks"` |
