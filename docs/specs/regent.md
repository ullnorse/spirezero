# Spec: Regent mechanics, and Ironclad/Silent gaps (S-MECH)

Research only, no engine code. Input for C17 (stars and Sovereign Blade core) and the Regent fan-out. Paths are under
`ref/decomp/MegaCrit.Sts2.Core.`; `CM` = `Models/CardModel.cs`, `H` = `Hooks/Hook.cs`, `AM` = `Models/AbstractModel.cs`,
`PCS` = `Entities.Players/PlayerCombatState.cs`, `PC` = `Commands/PlayerCmd.cs`. Engine paths are under `crates/sts2-core/src/`.
Sibling specs: `orbs.md`, `hooks.md`.

## 1. Character
`Models.Characters/Regent.cs`: 75 HP, 99 gold, `ShouldAlwaysShowStarCounter => true` (`:36`), `RegentCardPool`, relic `DivineRight`, deck 4
`StrikeRegent`, 4 `DefendRegent`, `FallingStar`, `Venerate` (`:44-58`). `ShouldAlwaysShowStarCounter` is **display only**: its two readers
are `Nodes.Combat/NStarCounter.cs:303` and `NCombatUi.cs:323`, the combat state never reads it. The engine needs nothing for it; only the
tracer's dump should always include `stars` for the Regent. Base orb slots 0 (default), energy 3.

## 2. Stars
* State: `PlayerCombatState.Stars` (`PCS:103-117`), an int, **0 at every combat start, no cap, no per-turn reset**. Only
  `HandlePlayerDeath` zeroes it (`Combat/CombatManager.cs:1229`). The setter logs `History.StarsModified(delta)` (`StarsModifiedEntry`,
  read by `Radiate`: sum of positive entries this turn by the owner, `Radiate.cs:25-27`).
* `PlayerCmd.GainStars(amount, p)` (`PC:89-96`): skipped when `IsEnding`; `Hook.ShouldGainStars` is a veto (all listeners must allow,
  `H:2339`; no model overrides it); `PCS.GainStars` (`PCS:220-226`: throws on negative, `Stars = (int)max(Stars + amount, 0)`);
  then `Hook.AfterStarsGained(int amount, player)` (`H:1192`).
* `PlayerCmd.LoseStars` (`PC:103-111`): skipped when `IsEnding`, **no hook**. `SetStars` = Gain or Lose by the difference (`PC:118-130`).
* **Spending** is `CardModel.SpendResources` (`CM:1807-1820`) on every play (manual plays; an auto-play pays nothing): energy first
  (`SpendEnergy`, `AfterEnergySpent` even for 0), then `SpendStars(amount)`: `LastStarsSpent = amount` (always), and if `amount > 0`
  `PCS.LoseStars(amount)` then `Hook.AfterStarsSpent(amount, owner)` (`CM:1836-1845`, `H:1204`). Stars are paid at the **start** of the play,
  before `OnPlay` (hence `BlackHole` reacts in `AfterCardPlayed`, not `AfterStarsSpent`: `BlackHolePower.cs:18`).
* Legality: `HasEnoughResourcesFor` (`PCS:190-209`) adds `UnplayableReason.StarCostTooHigh` when `max(0, starCost) > Stars`.
* Card cost: `CanonicalStarCost` (virtual, default **-1 = none**, `CM:410`), `BaseStarCost`, `CurrentStarCost` (last temporary cost wins, `CM:451`),
  `HasStarCostX` (`CM:468`). `GetStarCostWithModifiers` (`CM:1556-1568`): X cards return **all current stars**; a card in a combat pile folds
  `Hook.ModifyStarCost` (`H:2023`: skipped for cost < 0; each listener's `TryModifyStarCost(card, cost, out cost)`); else `CurrentStarCost`.
  Modifiers in the pool: `VoidFormPower:92` and `BrilliantScarf:68` set the cost to 0 for eligible cards (the same models zero the energy
  cost in `TryModifyEnergyCostInCombatLate`).
* Temporary star costs mirror energy costs: `SetStarCostUntilPlayed/ThisTurn/ThisCombat` (`CM:1279-1291`), cleared at `EndOfTurnCleanup` (`CM:1620`) and
  when the card is played (`CM:2011`). `SetToFreeThisTurn/ThisCombat` (`CM:1267-1277`) zero **both** costs (used by `Discovery`, `WhiteNoise`,
  `Metamorphosis`, `Abundance`, `Distraction`, `BulletTime`, `MadScience`, `Splash`; not ported yet).
* X star cost (`Stardust`): `ResolveStarXValue() = Hook.ModifyXValue(LastStarsSpent)` (`CM:1139`), so `ChemicalX` applies; hit count of a
  random-target attack. `ShouldPayExcessEnergyCostWithStars` (`H:2354`, `CM:1811`: pay `2 stars` per missing energy) has **no implementer** in the
  pool: a dead hook, do not port yet.
* Regent cards with a star cost (energy/type/rarity, stars): `CloakOfStars` 0 Skill C 1; `GuidingStar` 1 Atk C 1; `CrescentSpear` 1 Atk C 1;
  `FallingStar` 0 Atk Basic 2; `Constellation` 0 Skill U 2; `Alignment` 0 Skill U 2; `ParticleWall` 0 Skill U 2; `Resonance` 1 Skill U 2; `Quasar` 0 Skill U 2;
  `MeteorShower` 0 Atk Ancient 2; `AstralPulse` 0 Atk C 3; `GammaBlast` 0 Atk U 3; `Reflect` 1 Skill U 3; `DyingStar` 1 Atk R 3; `TheSealedThrone` 1 Power
  Ancient 3; `Devastate` 1 Atk U 4; `TheSmith` 1 Skill R 4; `Comet` 0 Atk R 5; `NeutronAegis` 1 Power R 5; `RoyalGamble` 0 Skill U 5;
  `DecisionsDecisions` 0 Skill R 6; `SevenStars` 2 Atk R 7; `Stardust` 0 Atk U X. `CrescentSpear` damage scales with cards in
  `AllCards` that have `CanonicalStarCost >= 0 || HasStarCostX`.
* Star **gain** (`StarsVar`): `Venerate` 2, `BigBang` 1, `ShiningStrike` 2, `SolarStrike` 1, `GatherLight` 1, `HiddenCache` 1, `Glow` 1,
  `RoyalGamble` 9, `KnockoutBlow` 5 (all `PlayerCmd.GainStars` in `OnPlay`); `Convergence`/`Radiate` use `StarsVar` for other effects.
  Power-driven: `StarNextTurnPower` (`AfterEnergyReset`: gain `Amount`, remove, `:14`), `GenesisPower` (`AfterEnergyReset`, `:14`),
  `TheSealedThronePower` (`BeforeCardPlayed` of any owner card, `:14`), relics `DivineRight` (3, `AfterRoomEntered` in a `CombatRoom`, `:16`),
  `LunarPastry` (1 at the owner's `AfterSideTurnEnd`, `:19`), `StarPotion` 3 (`:22`).
* Star **spend** reactions: `ChildOfTheStarsPower` (Unpowered block `Amount * spent`, `:15`), `GalacticDust` (counter; at `StarsSpent >= 10`, block
  `floor(n/10) * 10` Unpowered, remainder kept, saved property, `:88`), `MiniRegent` (1 Strength, once per turn, `:40`), `BlackHolePower` (`Amount`
  Unpowered damage to HE after a card that spent stars (last in series), and on each positive `AfterStarsGained`, `:18,29`).

## 3. Sovereign Blade and `ForgeCmd`
* `SovereignBlade` (`Models.Cards/SovereignBlade.cs`): `TokenCardPool`, Token rarity, cost 2 (`:118`), Attack, `AnyEnemy`, keyword **Retain**,
  `DamageVar(10)`, `RepeatVar(1)` (hits), upgrade = cost -1 (`:169`). `_currentDamage`/`_currentRepeats` mirror the vars so `AfterDowngraded`
  restores them (`:180`). `AfterCloned` clears `CreatedThroughForge` (`:174`, UI only).
* Dynamic target type (`:38`): `AllEnemies` when the owner has `SeekingEdgePower` (attack `TargetingAllOpponents`), else `AnyEnemy`.
  Dynamic `GainsBlock` (`:53`): owner's `ParryPower` amount > 0 and card in a combat pile; after the attack it gains
  `CalculatedBlock = ParryAmount` block with the card as source (`:122-166`). `ConquerorPower` doubles blade damage vs its owner
  (`ModifyDamageMultiplicative`, powered attacks, `:24`; ticks down at `AfterSideTurnEnd`). `SwordSagePower` adds `Amount` to every blade's
  `BaseReplayCount` (new non-clone cards too, `AfterCardEnteredCombat`, `:29,47`; undone `AfterRemoved`). `SummonForth` moves all non-Hand blades to the Hand.
* **`ForgeCmd.Forge(amount, player, source)`** (`Commands/ForgeCmd.cs:36-53`), skipped when over/ending:
  1. `blades = player's SovereignBlades in AllCards, not dupes, not in Exhaust` (a card in no pile counts).
  2. If none: `CreateCard<SovereignBlade>`, `CreatedThroughForge = true`, `CardPileCmd.AddGeneratedCardToCombat(blade, Hand)` (fires the
     generated-card hooks with `creator = player`: `Regalite`, `PillarOfCreation`, `Arsenal` react **only to this first forge**).
  3. `IncreaseSovereignBladeDamage`: `AddDamage(amount)` (`Damage.BaseValue += amount`) to **every** blade including exhausted ones, `AfterForged()`
     (event for UI). A fresh blade therefore has `10 + amount`; an exhausted old blade also keeps growing.
  4. `Hook.AfterForge(amount, forger, source)` (`H:586`, combat guard; `AM:1198`): `HammerTimePower` forges the same amount for the **other** players
     (`source is HammerTimePower` stops recursion; multiplayer only).
* Forge amounts (`ForgeVar`): `Bulwark` 10 (+3), `Conqueror` 3 (+2), `SeekingEdge` 7 (+4), `WroughtInWar` 7 (+2), `RefineBlade` 8 (+4), `TheSmith` 30 (+10),
  `SpoilsOfBattle` 6 (+3), `BigBang` 5, `SummonForth` 8 (+3), `Furnace` 5 per turn (`FurnacePower.AfterSideTurnStart`, `:20`), relic `FencingManual` 10 (turn 1,
  `AfterSideTurnStart`, `:21`), `KingsCourage` 15 (`:25`). `BeatIntoShape` forges `Calculated - hits*Extra`, where `Calculated` counts `DamageReceivedEntry`s this turn
  by this target from this player (powered attacks) times `CalculationExtra` (`BeatIntoShape.cs:45`, history query).

## 4. Royalties and the rest of the Regent kit
* `Royalties` (`Models.Cards/Royalties.cs:13`): Power, cost 1, Rare, `GoldVar(30)` (+10), `CanBeGeneratedInCombat => false`; applies `RoyaltiesPower` with the
  gold amount. `RoyaltiesPower.AfterCombatEnd(room)` (`:14`): `room.AddExtraReward(owner, new GoldReward(Amount, owner))`. `CombatRoom.AddExtraReward`
  (`Rooms/CombatRoom.cs:185`) appends to a per-player list; `RewardsSet` adds it after the generated rewards (`Rewards/RewardsSet.cs:96-99`). It is a fixed `GoldReward`
  (no `Rng`). Win only. The same channel serves `TheHunt` (Silent): an extra 3-card `CardReward`.
* Other Regent powers: `VoidForm` (first `Amount` plays each turn free; `PlayerCmd.EndTurn` from the card; `Data.cardsPlayedThisTurn`, `:81-103`),
  `Orbit` (every 4 energy spent returns `Amount` energy, instanced, `OrbitPower.cs:38`), `Reflect` (blocked powered-attack damage reflected, ticks at `AfterSideTurnStart`, `:19`), `Tyranny` (`ModifyHandDraw` +`Amount`, exhaust `Amount` at
  `AfterPlayerTurnStart`, `:21`), `Monologue` (Strength per card played, undone at `AfterSideTurnEnd`, `:70`), `Genesis`, `PillarOfCreation`, `Arsenal`
  (generation reactions), `ForegoneConclusion`, `SpectrumShift` (`BeforeHandDraw` colorless cards), `PaleBlueDot`, `MonarchsGaze`.
* Minion cards: `Charge` transforms `Draw` cards into `MinionDiveBomb` (`CardTag.Minion`); relic `VitruvianMinion` doubles damage/block from Minion-tag cards.
* Relics: `DivineRight` (starter), `Regalite` (4 block on the first generated card per turn), `OrangeDough` (2 colorless cards turn 1), `WhiteStar`,
  `BlackStar` (extra elite rewards, run layer, `WhiteStar` already ported).

## 5. Engine gaps (`crates/sts2-core`)
Status after C17: R1-R6, R8-R10 and the `StarsModified` half of R7 are done (`combat/star_cost.rs`, `commands/stars.rs`,
`commands/forge.rs`, `CombatState::extra_rewards`, `Engine::end_turn`, `CardModel::is_minion`, `CardModel::target_in`; facts in
`docs/semantics/combat-core.md`, C17). Ported: `FallingStar`, `Venerate`, `SovereignBlade`, `Bulwark`, `WroughtInWar`, `RefineBlade`, `SeekingEdge`,
`Parry`, `CloakOfStars`, `GatherLight`, `Stardust`, `VoidForm`, `Royalties`, their powers and `DivineRight`. `CardPlay::star_value`/`stars_spent()` carry
the stars of a play.

| # | Gap | Notes / size |
|---|---|---|
| R1 | `Player` has no `stars`; `CombatConfig` none. Stars reset to 0 each combat. | `i32`; `Character::Regent` shows stars |
| R2 | Commands: `gain_stars` (`IsEnding`, `ShouldGainStars`, `AfterStarsGained`), `lose_stars`, `set_stars`. | `combat/commands/stars.rs`, ~60 lines |
| R3 | `play_card_action` pays energy only (`card_play.rs:51-52` says "No in-scope card costs stars"); needs `SpendStars`, `LastStarsSpent`, legality `StarCostTooHigh` (`check_play`), and the `AfterStarsSpent` hook. | `CardPlay::resources` needs `stars_spent` for `BlackHole`/`IsLastInSeries` |
| R4 | Card star cost: `CardModel::canonical_star_cost() -> i32` (-1), `star_cost_x`, per-card temporary costs, `TryModifyStarCost` fold (`ModifyStarCost`), `SetToFree*` zeroing both. `energy_cost.rs` is the template. | ~150 lines |
| R5 | Hooks missing in `hooks.md`/`hooks.rs`: `AfterStarsGained`, `AfterStarsSpent`, `ShouldGainStars`, `TryModifyStarCost`, `AfterForge` (`ShouldPayExcess...` dead). | combat guard |
| R6 | `ForgeCmd` and `SovereignBlade`: a card with mutable damage/repeat/replay, dynamic target type and `gains_block`, `CardKind::SovereignBlade`, `Engine::forge`. `CardData` already holds per-instance fields. | ~120 lines; needs `AllCards` scan, dupe flag (`clone_of` is not `IsDupe`) |
| R7 | `History`: `StarsModified` entry (`Radiate`), `DamageReceived` (`BeatIntoShape`), per `history.rs:14`. | shared with Silent (section 6) |
| R8 | Combat-to-run extra rewards: no channel for an `AfterCombatEnd` power to add a reward (`Royalties`, `TheHunt`); `combat_extra_rewards` (`run/mod.rs:378`) only serves event fights. | `CombatState::extra_rewards` read by `win_combat` |
| R9 | `PlayerCmd.EndTurn` from a card (`VoidForm`): not a command (only `Action::EndTurn`, `combat/mod.rs:227`). | tiny |
| R10 | Tags: only `is_osty_attack`/`is_defend` (`content/cards/mod.rs:713-719`); `Minion` tag missing (`VitruvianMinion`, `Charge`). | add `is_minion` |
| R11 | Content: 91 Regent cards (22 with a star cost, plus `Stardust`), `SovereignBlade` + 3 `Minion*` tokens, 8 pool relics, `StarPotion`/`KingsCourage`/`BlessingOfTheForge`, ~30 powers. `BlessingOfTheForge` exists in the engine (upgrade, not Forge). | fan-out after C17 |
| R12 | Tracer/compare: dump `stars`, each blade's damage/repeats/replay; `ShouldAlwaysShowStarCounter` is display only. | T10 |

## 6. Ironclad and Silent: unwired hooks and missing commands
Method: every `override` in the 181 Ironclad/Silent cards, the 67 distinct powers they apply and the 16 pool relics, against `hooks.md` and the engine.
* **Hooks: all wired.** Every hook those models override (`AfterCardDiscarded`, `AfterCardExhausted`, `AfterCardDrawn(Early)`, `ShouldFlush`, `ShouldDraw`,
  `ModifyHpLostAfterOsty`, `ModifyDamageCap`, `ModifyCardPlayCount/ResultLocation`, `TryModifyEnergyCostInCombatLate`, `BeforeSideTurnEnd(Early)`, ...) has a `wired`
  row. Only two rows are not: `ShouldOwnerDeathTriggerFatal` (dispatcher exists, no card calls it) and `AfterDiedToDoom` (Necrobinder).
* **Fatal** (`Feed` Ironclad, `HandOfGreed` Colorless, `TheHunt` Silent): `cardPlay.Target.Powers.All(ShouldOwnerDeathTriggerFatal)` read **before** the attack, then
  `attackCommand.Results.Any(WasTargetKilled)`. `Engine::attack` returns the results and `DamageResult::was_target_killed` exists (`damage.rs:22`); missing: the card-side
  helper plus `GainMaxHp` (exists), `GainGold` (exists), and the extra card reward for `TheHunt` (R8).
* **Poison** (7 cards plus `Envenom`, `NoxiousFumes`, `Accelerant`, ...): `PoisonPower` is absent. Semantics (`Models.Powers/PoisonPower.cs`): debuff, counter;
  `AfterSideTurnStart` of the owner: `TriggerCount = min(Amount, 1 + sum of living opponents' AccelerantPower)`; per trigger `Damage(Owner, Amount,
  Unblockable|Unpowered, dealer null)`, then `Decrement` only if still alive. `CalculateTotalDamageNextTurn` (UI) folds `ModifyDamage` for each tick.
* **Self-damage / HP loss** (`Bloodletting`, `Offering`, `Hemokinesis`, `Rupture`): `CreatureCmd.Damage` with `ValueProp.Unblockable |
  Unpowered` and the player as target and dealer: the engine's `damage` supports both props (`ValueProps`); `Rupture` reacts in `AfterCurrentHpChanged`/`AfterDamageReceived`
  (wired). No new command; `AfterDamageReceived` must see `result.unblocked > 0`.
* **Exhaust synergies** (`FeelNoPain`, `DarkEmbrace`, `Corruption`, `EvilEye`, `Midnight`): hooks wired; `History::CardExhausted` is recorded. `Corruption` needs
  `ModifyCardPlayResultLocation` (wired) and `TryModifyEnergyCostInCombatLate` (wired); `EvilEye`/`Midnight` read "exhausted this turn" from history (indexed already).
* **Strength synergies**: `Strength`, `Vulnerable`, `Weak` exist; `Inflame`/`Rage` are plain powers; multipliers such as `Pinpoint`/`Stomp` use `CalculatedDamageVar` (exists, `damage.rs:41`). `DemonForm`/`Rage`/`Inflame`
  are content only.
* **Discard / Sly synergies** (`Tactician`, `Reflex`, `Abrasive`, `HandTrick`, `Sneaky`, `MasterPlanner`, ...): `Discard`, `DiscardAndDraw`, the Sly autoplay and
  `AfterCardDiscarded` are wired. `MementoMori` reads `CardDiscardedEntry` this turn (recorded).
* **Missing, in order of use:** (1) `DamageReceivedEntry` history (`Spite`, `TearAsunder`, `EmotionChip`, `BeatIntoShape`; `history.rs:14` lists it unrecorded).
  (2) `CardTag.Strike` (9 uses), `CardTag.Shiv` (2 + the Shiv token), `CardTag.Minion` (only `is_defend`/`is_osty_attack` exist). (3) The `Shiv` token card and
  the Shiv-generating cards (`BladeDance`, `CloakAndDagger`, `InfiniteBlades`, `HiddenDaggers`, `UpMySleeve`, `StormOfSteel`, `Accuracy` reads the tag in `ModifyDamageAdditive`).
  (4) Powers: 58 of the 67 powers those cards apply are absent (`Poison`, `Accelerant`, `Barricade`, `Intangible`, `Corruption`, `FeelNoPain`, `DarkEmbrace`,
  `NoDraw`, `Afterimage`, `Blur`, ...); `FadePower`/`PiercingWail` style temporaries follow `temporary_strength.rs`. (5) `CardSelectCmd.FromHandForUpgrade` / `FromHandForDiscard`
  prompts (6 uses): `choose_cards` exists, only the prompt kind differs. (6) `PowerCmd.Remove<ArtifactPower>` (`remove_power` exists). No new core hook is required.
