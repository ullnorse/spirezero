# Content gaps: relics, potions, events (S1)

Survey of what the Act 1 relics, potions and events need that the engine and run layer lack, judged
against `main` after RL3 (rewards, rest sites, treasure, `?` rooms). Method: `docs/inventory/act1.json`
hooks, commands and rng per entity, checked against `docs/specs/hooks.md` (status column), the combat
commands in `crates/sts2-core/src/combat/commands/`, and the run layer in `crates/sts2-core/src/run/`;
odd entities read in the decomp. Counts are a survey, ±10%: a few entities will turn up one more
small need when ported. Paths below are under `ref/decomp/MegaCrit.Sts2.Core.`.

## Scope and definitions
* **Reachable in Act 1**: pool relics (Shared and Necrobinder, rarity Common/Uncommon/Rare/Shop), the
  event relics Act 1 events hand out (`ChosenCheese`, `SwordOfStone`, `BoneTea`, `EmberTea`,
  `TeaOfDiscourtesy`), all 50 potions, and the 21 events that can occur in Act 1
  (`docs/specs/run-layer.md` §7.1). **Later**: 25 relics (the 10 `Fake*` relics, `BingBong`,
  `DaughterOfTheWind`, `MrStruggles`, `Wongo*`, `HistoryCourse`, `LastingCandy`, `RazorTooth`,
  `SparklingRouge`, `SwordOfJade`, `FresnelLens`, the Ancient `LoomingFruit`/`VeryHotCocoa`, and the
  other characters' `PaperKrane`/`PaperPhrog`) and 10 events (`CrystalSphere`, `DollRoom`, `FakeMerchant`,
  `PotionCourier`, `RanwidTheElder`, `RelicTrader`, `StoneOfAllTime`, `Symbiote`, `WelcomeToWongos`,
  `WarHistorianRepy`). They need the same two core tasks and later-act content.
* **Portable** means no new core feature is needed. It does not mean no other content is needed: each
  entity may still wait for powers, enchantments, cards or relics that nobody has ported yet. Those
  are content batches (section 5), not core gaps.
* Already ported (excluded from every count below): relics `BoundPhylactery`, `Strawberry`, `Pear`,
  `Mango`, `LeesWaffle`, `OldCoin`, `PotionBelt` (7); potions `FirePotion`, `BlockPotion`,
  `EnergyPotion` (3). The run layer also handles Neow-only relics (`LavaRock` and friends), which are
  outside this scope.

## 1. What already exists (do not re-plan)
| Mechanic | Where |
|---|---|
| Every combat hook any in-scope relic or potion overrides (turn, card, damage, block, power, death, energy, draw, flush, shuffle, potion, combat end) | `docs/specs/hooks.md`, all rows `wired` |
| Every run-level hook (RL5): `RelicModel` methods dispatched over the owned relics by `run/hooks.rs`; every owned relic is a `RelicKind` with its fields in `RunRelic::data` | `content/relics/mod.rs`, `run/hooks.rs`, `docs/specs/hooks.md` |
| Commands: `GainGold`, `GainBlock`, `Heal`, `GainMaxHp`, `Damage`, `Kill`, `GainEnergy`, `Draw`, `Discard`, `Shuffle`, `Upgrade`, `Enchant`, `Downgrade`, `TransformTo`, `ApplyKeyword`, `AddGeneratedCardToCombat`, `AddToPile`, `Create/CloneCard`, `ApplyPower`, `TryToProcure` (ported kinds), `DiscardPotion`, `Osty.Summon`, random target | `combat/commands/*.rs` |
| Player card choice in combat (`FromHand`, `FromHandForDiscard`, `FromCombatPile`) via `choose_cards` | `combat/engine.rs:220` |
| Card-cost setters (`add_until_played`, `set_this_combat`, ...), the combat rng streams (`CombatCardSelection`, `CombatCardGeneration`, `CombatTargets`, `CombatEnergyCosts`, `CombatPotionGeneration`, `Niche`) | `combat/energy_cost.rs`, `rng/sets.rs` |
| Run: rewards, potion belt, relic bags (`PullNextRelicFromFront`), treasure, rest site (Heal/Smith), `?` odds, Neow, deck upgrade/removal select, event visited list and `RoomSet` event order | `run/*.rs` |
| `ShouldDie`/`AfterPreventingDeath` (`FairyInABottle`, `LizardTail`) | wired |

## 2. Gap table
Status: **combat** = missing in the combat engine; **run** = missing in the run layer; **shop** = needs
the Shop room (`Room::Unsupported` today, run-layer spec §5, about 450 lines, a third task not
proposed here). Sizes are Rust lines including tests.

| # | Mechanic or command | Needed by | Status | Size |
|---|---|---|---|---|
| A1 | **Relic instance state in combat**, seeded from and written back to `RunRelic.state` (`RelicKind` is stateless: `Relic {id, kind}`). Per-combat counters and saved properties | 32 relics: `ArtOfWar`, `BeatingRemnant`, `BeltBuckle`, `BookOfFiveRings`, `BurningSticks`, `CentennialPuzzle`, `Girya`, `HappyFlower`, `JossPaper`, `Kunai`, `Kusarigama`, `LavaLamp`, `LetterOpener`, `LizardTail`, `Nunchaku`, `Orichalcum`, `OrnamentalFan`, `PenNib`, `Pendulum`, `Permafrost`, `Pocketwatch`, `RainbowRing`, `Shuriken`, `StoneCalendar`, `SwordOfStone`, `TuningFork`, `UnsettlingLamp`, `Vambrace`, `VenerableTeaSet`, `BoneTea`, `EmberTea`, `TeaOfDiscourtesy` | done (C9): `RelicData` on the combat `Relic`, `RelicModel::init_data`; the run keeps it in `RelicState::Combat` (`run/relics.rs` `combat_relics`/`write_back`). Proof `Nunchaku` | 250 |
| A2 | `PlayerCmd.LoseEnergy` | `Bread` | done (C9): `Engine::lose_energy`. Proof `Bread` | 30 |
| A3 | `CreatureCmd.LoseBlock` | `SturdyClamp` | done (C9): `Engine::lose_block`. Proof `SturdyClamp` | 30 |
| A4 | `CardCmd.AutoPlay`, `CardPileCmd.AutoPlayFromDrawPile` (card play without cost, target pick, `AfterAutoPrePlayPhaseEntered` already wired) | `DistilledChaos` (`HistoryCourse` is later) | done (C9): `Engine::auto_play_from_draw_pile` (`Position::Top`/`Bottom`), `shuffle_if_necessary`. Proof `DistilledChaos` | 200 |
| A5 | **Card generation for combat**: `CardFactory.GetDistinctForCombat` over the Necrobinder and Colorless pools (by type and rarity, on `CombatCardGeneration`), plus `FromChooseACardScreen` on cards that are not in a pile (pick one, discard the rest). The Necrobinder pool exists as `run/cards.rs`; the Colorless pool and a generation entry point do not. Generated cards must be ported `CardKind`s | `BigHat`, `VexingPuzzlebox`, `Toolbox`, `AttackPotion`, `SkillPotion`, `PowerPotion`, `ColorlessPotion`, `OrobicAcid` | done (C9): `content/cards/pools.rs` (Necrobinder and Colorless pools with the filter data of unported cards), `Engine::get_distinct_for_combat` (panics on an unported pick), `Engine::choose_a_card`. `BigHat` ported for combat only (5 of its 8 Ethereal cards are unported; the run refuses it) | 250 |
| A6 | Potion belt of ids in combat (a generated potion that is not a ported `PotionKind` must still occupy a slot) | `EntropicBrew` | **done (RL5)**: the combat belt has the run's slots (`CombatConfig::potions` of `BeltPotion`, `Player::unported_potions`, `first_empty_potion_slot`); the run reads it back by slot. Left: a by-id procure for an in-combat generator that draws an unported potion (`EntropicBrew` in combat) | 120 |
| B1 | **Event framework**: `Room::Event`, `RunDecision::Event(options)`, `RunAction::EventOption`, pages (`SetEventState`/`SetEventFinished`), `PullNextEvent` with `IsAllowed` and `ModifyNextEvent`, per-event `Rng` (`runSeed + slot + xxh64(ID)`, as Neow), `CalculateVars`, `BeforeEventStarted`/`OnEventFinished`/`OnRoomEnter`, `eventsVisited` bookkeeping, history entry | all 21 Act 1 events | **done (RL4)**: `run/events.rs`, `content/events/` | 600 |
| B2 | **Run-side commands**: `LoseGold`, `LoseMaxHp`, run `Damage`/`Heal`/`Kill` of the player, `RemoveFromDeck` (random and chosen), `AddCurse(s)ToDeck`, `TransformToRandom` (rng passed in) and `TransformTo`, `Downgrade`, `Enchant` on deck cards, `PotionCmd.Discard`, `RelicCmd.Remove`/`Replace`/`Obtain<T>`, `MimicRestSiteHeal`, `CompleteQuest` skipped (`WarHistorianRepy` never occurs) | 20 events, `DollysMirror`, `SwordOfStone`, `GnarledHammer`, `Kifuda`, `PunchDagger`, `RoyalStamp`, `WarPaint`, `Whetstone`, `Cauldron`, `Orrery` | **partly done (RL4)**: `RunState::{gain_gold, lose_gold, damage, heal, lose_max_hp, kill, add_card, remove_card, discard_potion}`, relic obtain and `PullNextRelicFromFront` from events. **RL5**: `RunState::{transform_to_random, transform_to, downgrade, enchant, remove_cards, gain_max_potion_count}`, `RunCtx::{remove_relic, replace_relic}`. Left: `MimicRestSiteHeal` (`DenseVegetation`) | 500 |
| B3 | **Deck card metadata beyond the Necrobinder pool**: `RunCard` knows only pool cards (`info()` is `None` for curses, status, colorless; `is_upgradable` is false for them) and has no enchantment. Needs type, rarity, `MaxUpgradeLevel`, `IsRemovable`, transformable flag, quest/curse flags, and an enchantment slot carried into combat | curse events (`Guilty`, `Clumsy`, `SporeMind`, `PoorSleep`), transform events, 5 enchant events/relics, `ByrdonisNest` (`ByrdonisEgg`), `TheLegendsWereTrue` (`SpoilsMap`) | **partly done (RL4)**: `RunCard::{is_basic, is_removable, card_type}`, the colorless pool, curse and quest cards (`MaxUpgradeLevel` 0). **RL5**: `RunCard::enchantment`, carried into combat (`DeckCard::enchantment`), `RunCard::can_enchant` | 250 |
| B4 | **Card select variants**: `SelectPurpose` has only Upgrade/Smith/Remove. Add transform, enchant (with `CanEnchant` filter and `Cancelable`), generic (`FromDeckGeneric`), multi-card remove from events, and a "pick 1 of N shown cards" screen (`FromChooseACardScreen` out of combat) | `WoodCarvings`, `Wellspring`, `LuminousChoir`, `MorphicGrove`, `AromaOfChaos`, `SapphireSeed`, `SelfHelpBook`, `WhisperingHollow`, `DollysMirror`, enchant relics | **partly done (RL4)**: `SelectPurpose::AddToDeck` (pick N of shown cards, `SelectCardsToAddToDeckFromGrid`). **RL5**: `RunCtx::select(SelectPurpose, options, count)` with `RunState::{upgradable,removable,transformable,enchantable}_cards`, `SelectPurpose::Enchant`, and `EventCtx::select_deck_cards` (an event's own use of the picks: removal, transform, generic). Left: `Cancelable` enchant selections | 200 |
| B5 | **Run-level relic hook dispatch** (**done, RL5**: `RelicModel`'s run section, `run/hooks.rs`; rows in `hooks.md` are `wired` except the shop and map ones). `Hooks` methods with no call site: `TryModifyRewards`/`AfterModifyingRewards`/`BeforeCombatRewardOffered`/`ShouldForcePotionReward`, `TryModifyCardRewardOptions(+Late)`/`ModifyCardRewardCreationOptions`/`TryModifyCardBeingAddedToDeck(+Late)`, rest-site set (`TryModifyRestSiteOptions`, `ShouldDisableRemainingRestSiteOptions`, `ModifyRestSiteHealAmount`, `ModifyExtraRestSiteHealText`, `TryModifyRestSiteHealRewards`, `AfterRestSiteHeal`), gold (`ModifyGoldGained`, `AfterModifyingGoldGained`, `AfterGoldGained`), `ModifyUnknownMapPointRoomTypes`, and `AfterRoomEntered`/`AfterObtained` for any room and relic | 28 relics: `AmethystAubergine`, `PrayerWheel`, `WhiteStar`, `WhiteBeastStatue`, `DingyRug`, `FrozenEgg`, `MoltenEgg`, `ToxicEgg`, `LavaLamp`, `WingCharm`, `BowlerHat`, `DragonFruit`, `JuzuBracelet`, `EternalFeather`, `Planisphere`, `Pantograph`, `RegalPillow`, `MiniatureTent`, `Shovel`, `TinyMailbox`, `Girya`, `VenerableTeaSet`, `Cauldron`, `Orrery`, ... | done (RL5). Proof: `PrayerWheel`, `DragonFruit`, `AmethystAubergine`, `WhiteStar`, `JuzuBracelet`, `RegalPillow`, `EternalFeather`, `BowlerHat`. `Pantograph` needs the room type in combat (as `BoomingConch`); `MiniatureTent` has no oracle (sts2-cli leaves the rest site) | 500 |
| B6 | **Custom rewards** (`RewardsCmd.OfferCustom`): `Reward` has Gold/Potion/Relic/Card; add the builders events use (potion from a uniform draw, card reward with fixed options, removal of a claimed relic reward) | `Wellspring`, `BrainLeech`, `TheLegendsWereTrue`, `TheFutureOfPotions`, `WhisperingHollow`, `Cauldron`, `Orrery` | **done (RL4)**: `EventCtx::offer_rewards`; `CardCreation` (pool, odds, filter, `IsCardReward`) for card rewards of fixed options, uniform potion draws | 150 |
| B7 | **Potion use outside combat** (`PotionUsage::AnyTime`): `RunAction::UsePotion`, target-less `OnUse` against `RunState` | `FruitJuice` (max HP), `EntropicBrew`, `FoulPotion` (later) | **done (RL5)**: `RunAction::UsePotion(slot)`, `PotionModel::on_use_out_of_combat`. Proof `FruitJuice` (unit test only: sts2-cli gives an `AnyPlayer` potion no target outside combat) | 120 |
| B8 | **Event-started combat** (`EnterCombatWithoutExitingEvent`, resume or not). Correction: the fight gives its encounter room's rewards plus `extraRewards` (`docs/semantics/run.md`, RL5). The referee never enters these fights, so this has no oracle | `DenseVegetation` (also needs its encounter) | **done (RL5)**: `EventCtx::enter_combat(encounter, extra_rewards, resume)`; unit-tested with a test event | 150 |
| S | Shop room and merchant hooks (`ModifyMerchantPrice`, `ModifyMerchantCardCreationResults`, `ShouldRefillMerchantEntry`, `AfterRoomEntered` in a shop) | `MealTicket`, `MembershipCard`, `TheCourier`; shop halves of the 4 card-option relics | **done (RL6)**: `run/shop.rs`, the 3 shop relics ported; `ModifyMerchantCardCreationResults` waits for the eggs and `FresnelLens` | 450+ |

## 3. Proposed core tasks
**Core task 1 (combat-side), about 880 lines: A1-A6.** Done in C9 except A6, which slipped to
task 2: its run half (`merge_belt`, the combat belt built from the run's slot list) is in
`run/mod.rs`, and the combat belt today packs the ported potions and treats the slots of unported
ones as empty. Order inside: A1 (unblocks 32 relics, mostly
independent of the rest), A2/A3/A4 (small, in `combat/commands/`), A5, A6 last (only `EntropicBrew`
needs it; it can slip to task 2). Done when each gap has a unit test through a toy relic, potion or
card, the way `test_choice.rs` exercises `choose_cards`. Touches `crates/sts2-core/src/combat/`,
`content/relics/mod.rs`, `content/potions/mod.rs`, and `run/relics.rs` for the write-back.

**Core task 2 (run-side), about 2,500 lines: B1-B8.** Done: milestone (i) in RL4, milestone (ii) and A6 in
RL5. Split into two reviewable milestones in one
brief: (i) B1 + B2 + B3 + B4 + B6 (events usable; unblocks 17 of the 21 Act 1 events), (ii) B5 + B7 + B8 (hooks and potion use; unblocks the 28 hook
relics, `FruitJuice`, `DenseVegetation`). Tasks 1 and 2 touch different directories and can run in
parallel; the 4 relics tagged A+B (`Girya`, `LavaLamp`, `SwordOfStone`, `VenerableTeaSet`) and
`EntropicBrew` need both.

Neither task needs `crates/sts2-core` combat hooks changed: every hook is already wired. B5 only adds
call sites in the run layer, so `docs/specs/hooks.md` rows change from `run layer` to `wired`.

## 4. Portable counts
Reachable in Act 1, not counting the 7 relics and 3 potions already ported. "After 1" and "after 2"
are cumulative in that order; the right column is the count if only task 2 were done.

| Kind | In scope | Portable today | After task 1 | After tasks 1+2 | Task 2 only |
|---|---:|---:|---:|---:|---:|
| Relics (Act 1) | 118 | 50 | 83 | 115 | 78 |
| Potions | 47 | 38 | 44 | 47 | 40 |
| Events (Act 1) | 21 | 0 | 0 | 21 | 21 |
| Later relics / events | 25 / 10 | 0 | 0 | after later-act content | same |

Left out of the "after 2" relic count: 3 shop relics (`MealTicket`, `MembershipCard`, `TheCourier`).
Of the 21 events, 4 also wait on content: `SapphireSeed`, `SelfHelpBook` and `WoodCarvings` on the
enchantments and cards in section 5, `DenseVegetation` on its encounter. Three events that pull
`PullNextRelicFromFront` relics (`LuminousChoir`, `ThisOrThat`, `UnrestSite`) hit
`RunError::UnsupportedRelic` until the pulled relic is ported, so port them after most pool relics.

## 5. Content prerequisites (not core gaps)
* **Powers (19 missing, 5 ported)**: relics: `Weak`, `Dexterity`, `Thorns`, `Plating`,
  `ReptileTrinket`. Potions: `Clarity`, `Duplication`, `FlexPotion`, `Gigantification`, `Buffer`,
  `Ritual`, `Doom`, `Demise`, `Radiance`, `Regen`, `ShacklingPotion`, `BlockNextTurn`, `SpeedPotion`,
  `RetainHand`. (`Confused` only for the later `FakeSneckoEye`.) Port these as batches P1 (relic
  powers, 5) and P2 (potion powers, 14) before the dependent entities; they need no core change.
* **Enchantments** (7 missing, 2 ported): `Nimble`, `Sharp`, `Swift`, `Momentum`, `Adroit`,
  `RoyallyApproved`, `Sown` (`Corrupted` is later). Needed by `WingCharm`, `GnarledHammer`, `Kifuda`,
  `PunchDagger`, `RoyalStamp`, `SapphireSeed`, `SelfHelpBook`.
* **Cards** events add: `ByrdonisEgg`, `SpoilsMap`, curses `Guilty`, `Clumsy`, `SporeMind`, `PoorSleep`,
  `Peck`, `ToricToughness`.
* **Potion tokens**: `PotionShapedRock` (for `PetrifiedToad`, portable today as content).

## 6. Porting batches (about 15 each, dependency order)
Each batch ships with its loadout files for `tools/regress.sh`. P1/P2 are the power batches above.

**Available today**
* **RT1** (15 relics): `Akabeko`, `Anchor`, `BagOfMarbles`, `BagOfPreparation`, `Bellows`, `BloodVial`,
  `BoneFlute`, `BookRepairKnife`, `Bookmark`, `Candelabra`, `CaptainsWheel`, `Chandelier`, `ChemicalX`,
  `CloakClasp`, `FestivePopper`.
* **RT2** (15): `FuneraryMask`, `GamblingChip`, `GamePiece`, `GhostSeed`, `GremlinHorn`, `HornCleat`,
  `IceCream`, `IntimidatingHelmet`, `IvoryTile`, `Lantern`, `LuckyFysh`, `MeatOnTheBone`,
  `MercuryHourglass`, `MiniatureCannon`, `MummifiedHand`.
* **RT3** (13): `MysticLighter`, `ParryingShield`, `RingingTriangle`, `RippleBasin`, `ScreamingFlagon`,
  `SlingOfCourage`, `StoneCracker`, `StrikeDummy`, `TheAbacus`, `TungstenRod`, `UnceasingTop`,
  `UndyingSigil`, `Vajra`. Plus `ChosenCheese` and, after `PotionShapedRock`, `PetrifiedToad`.
* **PT1** (18 potions, no new power): `ExplosiveAmpoule`, `Fortifier`, `StrengthPotion`,
  `VulnerablePotion`, `SwiftPotion`, `CureAll`, `BlessingOfTheForge`, `BottledPotential`,
  `LiquidMemories`, `DropletOfPrecognition`, `TouchOfInsanity`, `GamblersBrew`, `SneckoOil`,
  `FairyInABottle`, `BeetleJuice`, `BoneBrew`, `PotOfGhouls`, `PotionShapedRock`.
* After **P1**: **RT4** (5): `BronzeScales`, `Gorget`, `OddlySmoothStone`, `RedMask`, `ReptileTrinket`.
* After **P2**: **PT2** (20): `Clarity`, `DexterityPotion`, `Duplicator`, `FlexPotion`, `FyshOil`,
  `GigantificationPotion`, `HeartOfIron`, `LiquidBronze`, `LuckyTonic`, `MazalethsGift`,
  `PotionOfBinding`, `PotionOfDoom`, `PowderedDemise`, `RadiantTincture`, `RegenPotion`,
  `ShacklingPotion`, `ShipInABottle`, `SpeedPotion`, `StableSerum`, `WeakPotion`.

**After core task 1**
* **RA1** (15 relics with state): `ArtOfWar`, `BeatingRemnant`, `BookOfFiveRings`, `BurningSticks`,
  `CentennialPuzzle`, `HappyFlower`, `JossPaper`, `Kusarigama`, `LetterOpener`, `LizardTail`,
  `Nunchaku`, `Orichalcum`, `OrnamentalFan`, `PenNib`, `Pendulum`.
* **RA2** (15): `Permafrost`, `Pocketwatch`, `Shuriken`, `StoneCalendar`, `UnsettlingLamp`, `Vambrace`,
  `TuningFork`, `BoneTea`, `EmberTea`, `TeaOfDiscourtesy`, `Bread`, `SturdyClamp`, `BigHat`, `Toolbox`,
  `VexingPuzzlebox`.
* **RA3** (after P1): `BeltBuckle`, `Kunai`, `RainbowRing`.
* **PA1** (6 potions): `AttackPotion`, `SkillPotion`, `PowerPotion`, `ColorlessPotion`, `OrobicAcid`,
  `DistilledChaos`.

**After core task 2 (milestone i, events)**
* **E1** (12 events, no card select, no enchantment): ported in RL4 except `TeaMaster` (every option
  obtains a tea; `BoneTea`, `EmberTea`, `TeaOfDiscourtesy` wait for RA2): `JungleMazeAdventure`,
  `SunkenStatue`, `RoomFullOfCheese`, `ThisOrThat`, `UnrestSite`, `TabletOfTruth`, `SlipperyBridge`,
  `BrainLeech`, `TheLegendsWereTrue`, `ByrdonisNest`, `TheFutureOfPotions`. Options that obtain an unported
  relic (`SwordOfStone`, `ChosenCheese`, a pulled relic) or add an unported card (`ByrdonisEgg`,
  `SpoilsMap`, curses) are explicit errors or stop the diff policy until that content lands.
* **E2** (5, deck select or transform): `Wellspring`, `LuminousChoir`, `MorphicGrove`, `AromaOfChaos`,
  `WhisperingHollow`. `Wellspring` and `MorphicGrove` ported in RL5.
* **E3** (3, enchantments, after section 5): `SapphireSeed`, `SelfHelpBook`, `WoodCarvings`.

**After core task 2 (milestone ii, hooks)**
* **RB1** (15 relics): `AmethystAubergine`, `PrayerWheel`, `WhiteStar`, `WhiteBeastStatue`, `DingyRug`,
  `FrozenEgg`, `MoltenEgg`, `ToxicEgg`, `WingCharm` (needs `Swift`), `BowlerHat`, `DragonFruit`,
  `JuzuBracelet`, `EternalFeather`, `Planisphere`, `Pantograph`. Ported in RL5: `AmethystAubergine`,
  `PrayerWheel`, `WhiteStar`, `BowlerHat`, `DragonFruit`, `JuzuBracelet`, `EternalFeather`, plus
  `RegalPillow` (RB2). `Pantograph` waits for the room type in combat.
* **RB2** (13): `RegalPillow`, `MiniatureTent`, `Shovel`, `TinyMailbox`, `Cauldron`, `Orrery`,
  `WarPaint`, `Whetstone`, `DollysMirror`, `GnarledHammer`, `Kifuda`, `PunchDagger`, `RoyalStamp`.
* **RAB** (needs both tasks): `Girya`, `LavaLamp`, `SwordOfStone`, `VenerableTeaSet`.
* **PB1**: `FruitJuice` (ported in RL5); `EntropicBrew` (needs a by-id procure in combat).
* **E4**: `DenseVegetation` (needs its encounter and B8).

## 7. Risks and notes
* The referee cannot oracle event-started fights, auto-claims rewards and auto-picks the treasure relic
  (`tasks/BACKLOG.md`); events are otherwise drivable (`choose_option`, `select_cards`,
  `enter_room {type: event, event: ID}`), so use the forced-event entry for event diffs.
* A5 is bounded by what is ported: a generated card that is not a `CardKind` cannot enter a combat.
  The rng draws still follow the full pool, so port the draw first and restrict test seeds to
  ported cards until the pools are covered.
* A1 (done, C9): a `RelicData` enum like `PowerData`. All of a relic's fields outlive the combat,
  saved or not (one instance per run; relics reset their own counters in hooks), so the whole
  variant round-trips through the run.
* The `AfterRoomEntered` relics that act in non-combat rooms (`EternalFeather`,
  `VenerableTeaSet`, `RegalPillow`, `Planisphere`, `MealTicket`) were found by reading the decomp;
  `hooks.rs` only dispatches it at combat start.
