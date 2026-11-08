# Hook table

Core-only reference, split out of `content-api.md`. Content porters only need it to find where a hook fires.

Every hook the inventory uses (`cargo xtask inventory`), plus the other passes at the same call
sites. Paths: call sites under `crates/sts2-core/src/combat/` (`turn.rs`, `card_play.rs`,
`potion_use.rs`, `mod.rs`, `state.rs`, `hooks.rs`, `commands/*.rs`, `content/monsters/mod.rs`); decomp
under `ref/decomp/MegaCrit.Sts2.Core.`. Status: **wired** (called by the engine at that place),
**content call site** (the dispatcher exists; the model that calls it is not ported), **command missing**
(the dispatcher or method exists; the command that triggers it is not in the engine), **run layer**
(out of combat: a `Hooks` method with no call site and only the arguments the engine can express).
Guards follow `Hook.cs`: `IRunState` hooks and the documented direct ones are unguarded, the rest go
silent once combat is over or ending, except while combat is starting. A card's affliction and then
its enchantment listen right after the card (`Listener::Affliction`, `Listener::Enchantment`).
Hooks that fire outside combat are `RelicModel` methods (`content/relics/mod.rs`, run section), dispatched
over the owned relics in order by `run/hooks.rs` (`IterateHookListeners(null)`: deck cards, relics, potions;
only relics have run-level hooks among the ported models). A hook the combat engine also dispatches has an
`_out_of_combat` twin there.

| Hook | When it fires | Call site | Decomp | Status |
|---|---|---|---|---|
| `AfterRoomEntered` | run-level, after set-up, before combat start (relics, potions) | `turn.rs:start_combat` -> `hooks.rs:after_room_entered`; the run-state half (gold, run HP: `Planisphere`, `MawBank`) of a map fight room runs before the fight is set up, `run/mod.rs:enter_combat` -> `run/hooks.rs:after_map_combat_room_entered` (an event's own fight is not the base room, so it gets none) | `Rooms/CombatRoom.cs:225`, `Hook.cs:1128` | wired |
| `AfterAddedToRoom` | monster method, per enemy at combat start, before its first roll; also for an enemy added mid-combat | `turn.rs:start_combat`, `creature.rs:add_enemy` -> `monsters/mod.rs:after_added_to_room` | `Combat/CombatManager.cs:1125-1132` | wired |
| `AfterCreatureAddedToCombat` | after `CreatureCmd.Add` (unguarded) | `creature.rs:add_enemy` | `Commands/CreatureCmd.cs:81`, `Hook.cs:374` | wired |
| `BeforeCombatStart(+Late)` | once combat is in progress, before turn 1 | `turn.rs:start_combat` | `Combat/CombatManager.cs:594`, `Hook.cs:311` | wired |
| `BeforeSideTurnStart` | each side's turn start, after the power snapshot | `turn.rs:start_turn` | `Combat/CombatManager.cs:721`, `Hook.cs:1156` | wired |
| `ShouldClearBlock` | per creature starting its turn (not a player on turn 1) | `turn.rs:after_turn_start` | `Entities.Creatures/Creature.cs:686-733`, `Hook.cs:2201` | wired |
| `AfterPreventingBlockClear` | the ShouldClearBlock preventer only | `turn.rs:after_turn_start` | `Entities.Creatures/Creature.cs:731`, `Hook.cs:1044` | wired |
| `AfterBlockCleared` | per creature starting its turn, after every block clear, even if kept | `turn.rs:start_turn` | `Combat/CombatManager.cs:768`, `Hook.cs:119` | wired |
| `ShouldPlayerResetEnergy` | player turn start: reset to (or add) max energy | `turn.rs:setup_player_turn` | `Combat/CombatManager.cs:889`, `Hook.cs:2386` | wired |
| `ModifyMaxEnergy` | whenever max energy is read (`CombatState::max_energy`) | `hooks.rs:max_energy` (from `turn.rs:setup_player_turn`) | `Entities.Players/PlayerCombatState.cs:101`, `Hook.cs:1778` | wired |
| `AfterEnergyReset(+Late)` | after the energy reset | `turn.rs:setup_player_turn` | `Combat/CombatManager.cs:898`, `Hook.cs:515` | wired |
| `BeforeHandDraw(+Late)` | before the turn's hand draw | `turn.rs:setup_player_turn` | `Combat/CombatManager.cs:900`, `Hook.cs:600` | wired |
| `ModifyHandDraw(+Late)` | the hand-draw count (base 5) | `turn.rs:setup_player_turn` | `Combat/CombatManager.cs:902`, `Hook.cs:1692` | wired |
| `AfterModifyingHandDraw` | listeners that changed `(int)` hand draw | `turn.rs:setup_player_turn` | `Combat/CombatManager.cs:903`, `Hook.cs:751` | wired |
| `ModifyShuffleOrder` | a reshuffle's shuffled list, before it goes to the draw pile (the initial shuffle's only listener, `PerfectFit`, ignores it) | `piles.rs:shuffle` | `Commands/CardPileCmd.cs:1089`, `Hook.cs:2012` | wired |
| `AfterPlayerTurnStart(Early,Late)` | after the hand draw | `turn.rs:setup_player_turn` | `Combat/CombatManager.cs:923`, `Hook.cs:894` | wired |
| `AfterSideTurnStart(+Late)` | after the player set-up (both sides) | `turn.rs:start_turn` | `Combat/CombatManager.cs:783`, `Hook.cs:1175` | wired |
| `AfterAutoPrePlayPhaseEntered(Early,Late)` | player turn, after the empty-hand check, before the play phase | `turn.rs:start_turn` | `Combat/CombatManager.cs:864`, `Hook.cs:940` | wired |
| `AfterHandEmptied` | `CheckForEmptyHand`: after a card play, a potion use and the auto-pre-play entry | `turn.rs:check_for_empty_hand` | `Combat/CombatManager.cs:1175`, `Hook.cs:623` | wired |
| `AfterAutoPostPlayPhaseEntered` | end turn, phase one start | `turn.rs:end_player_turn_phase_one` | `Combat/CombatManager.cs:1542`, `Hook.cs:922` | wired |
| `BeforeSideTurnEnd(VeryEarly,Early)` | end of either side's turn, before turn-end cards / enemy cleanup | `turn.rs:end_player_turn_phase_one`, `turn.rs:execute_enemy_turn` | `Combat/CombatManager.cs:1556,1699`, `Hook.cs:1244` | wired |
| `OnTurnEndInHand` | card method: hand cards with `HasTurnEndInHandEffect`, via the Play pile | `turn.rs:do_turn_end` | `Combat/CombatManager.cs:1599-1690` | wired |
| `ShouldEtherealTrigger` | Ethereal hand cards at turn end | `turn.rs:do_turn_end` | `Combat/CombatManager.cs:1615`, `Hook.cs:2294` | wired |
| `BeforeFlush(+Late)` | end of phase one | `turn.rs:end_player_turn_phase_one` | `Combat/CombatManager.cs:1584`, `Hook.cs:544` | wired |
| `ShouldTakeExtraTurn` / `AfterTakingExtraTurn` | player turn end, after phase two: `SwitchFromPlayerToEnemySide` | `turn.rs:switch_from_player_to_enemy_side` | `Combat/CombatManager.cs:1825-1853`, `Hook.cs:1232,2465` | wired (single player) |
| `ShouldFlush` | phase two, before the hand goes to the discard pile | `piles.rs:flush_hand` | `Combat/CombatManager.cs:1799`, `Hook.cs:2309` | wired |
| `ShouldProcurePotion` | `PotionCmd.TryToProcure`, before the potion joins a slot (in combat, rewards, shop, events) | `commands/potion.rs:try_to_procure_potion`, `run/state.rs:procure_potion` | `Commands/PotionCmd.cs:27-52`, `Hook.cs:2416` | wired (`Sozu`) |
| `AfterFlush` | after the hand flush | `piles.rs:flush_hand` | `Combat/CombatManager.cs:1816`, `Hook.cs:572` | wired |
| `AfterSideTurnEnd(+Late)` | end of either side's turn, last | `turn.rs:end_player_turn_phase_two`, `turn.rs:execute_enemy_turn` | `Combat/CombatManager.cs:1779,1704`, `Hook.cs:1279` | wired |
| `TryModifyEnergyCostInCombat(+Late)` | a card's cost (`CombatState::energy_cost`): legality and spending | `hooks.rs:energy_cost` (from `mod.rs:check_play`, `card_play.rs:play_card_action`) | `Entities.Cards/CardEnergyCost.cs:94-123`, `Hook.cs:1582` | wired |
| `ShouldPlay` | `CanPlay` of a manual play | `mod.rs:check_play` | `Models/CardModel.cs:1746`, `Hook.cs:2369` | wired |
| `AfterEnergySpent` | after paying a card (also 0) | `card_play.rs:play_card_action` | `Models/CardModel.cs:1833`, `Hook.cs:532` | wired |
| `AfterStarsSpent` | after a manual play paid `amount > 0` stars (after `AfterEnergySpent`) | `card_play.rs:play_card_action` | `Models/CardModel.cs:1836-1845`, `Hook.cs:1204` | wired (no ported listener) |
| `ShouldGainStars` | veto before `PlayerCmd.GainStars` | `stars.rs:gain_stars` | `Commands/PlayerCmd.cs:91`, `Hook.cs:2339` | wired (no listener in the pool) |
| `AfterStarsGained` | after `PlayerCmd.GainStars` changed the stars | `stars.rs:gain_stars` | `Commands/PlayerCmd.cs:94`, `Hook.cs:1192` | wired (no ported listener) |
| `TryModifyStarCost` (`ModifyStarCost`) | `GetStarCostWithModifiers` of a card in a combat pile (not for cost < 0) | `hooks.rs:star_cost` | `Models/CardModel.cs:1556-1568`, `Hook.cs:2023` | wired (`VoidFormPower`) |
| `TryModifyKeywordsInCombat` (`ModifyKeywordsInCombat`) | every read of a combat card's `Keywords` (unguarded): Innate at combat start, Ethereal at turn end and `CardPlayFinished`, Exhaust result pile, Unplayable, Retain flush, Sly, keyword filters in content, the referee dump | `hooks.rs:keywords` (`CombatState::keywords`/`has_keyword`) | `Models/CardModel.cs:1153-1181`, `Hook.cs:1603`, `Models/AbstractModel.cs:2066` | wired (test-only `TestHex`; `HexPower` in GM2) |
| `AfterForge` | after `ForgeCmd.Forge` grew the blades | `forge.rs:forge` | `Commands/ForgeCmd.cs:52`, `Hook.cs:586` | wired (no ported listener) |
| `ShouldPayExcessEnergyCostWithStars` | legality and `SpendResources` | - | `Hook.cs:2354` | dead: no implementer in the pool |
| `GetResultLocationForCardPlay` | card-class virtual, the played card's own result location, before `ModifyCardPlayResultLocation` | `card_play.rs:on_play_wrapper` -> `CardModel::result_location_for_card_play` | `Models/CardModel.cs:1881,2084` | wired (C21) |
| `ModifyCardPlayResultLocation` | pile and position a played card goes to, given the play's resources (`NostalgiaPower`, `FeralPower`) | `card_play.rs:on_play_wrapper` | `Models/CardModel.cs:1882`, `Hook.cs:1403` | wired (C12; resources C23) |
| `AfterModifyingCardPlayResultLocation` | listeners that changed the result location, called directly, unguarded | `card_play.rs:on_play_wrapper` | `Models/CardModel.cs:1883-1886` | wired (C12) |
| `ModifyCardPlayCount` | play count of a card play | `card_play.rs:on_play_wrapper` | `Models/CardModel.cs:2032`, `Hook.cs:1384` | wired |
| `AfterModifyingCardPlayCount` | listeners that changed the play count | `card_play.rs:on_play_wrapper` | `Models/CardModel.cs:2033`, `Hook.cs:676` | wired |
| `BeforeCardPlayed` | each play, before `OnPlay` | `card_play.rs:play_loop` | `Models/CardModel.cs:1926`, `Hook.cs:263` | wired |
| `AfterCardPlayed(+Late)` | each play, after `OnPlay` (unguarded) | `card_play.rs:play_loop` | `Models/CardModel.cs:1965`, `Hook.cs:278` | wired |
| `AfterCardDiscarded` | `CardCmd.Discard`, per card, before Sly autoplays | `card.rs:discard` | `Commands/CardCmd.cs:194`, `Hook.cs:186` | wired |
| `AfterCardEnteredCombat` | a card added to a combat pile from no pile; a transform's replacement | `piles.rs:add_cards_to_pile`, `card.rs:transform_to` | `Commands/CardPileCmd.cs:514`, `Commands/CardCmd.cs:449`, `Hook.cs:223` | wired |
| `AfterCardChangedPiles(+Late)` | after each `CardPileCmd.Add` batch, per card whose pile type changed; manual play to Play; a transform's replacement | `piles.rs:add_cards_to_pile`, `card_play.rs:on_play_wrapper`, `card.rs:transform_to` | `Commands/CardPileCmd.cs:540,866`, `Commands/CardCmd.cs:451`, `Hook.cs:167` | wired |
| `AfterCreated` | card method, right after `CombatState.CreateCard` | `piles.rs:create_card` | `Combat/CombatState.cs:174-181` | wired |
| `AfterCardGeneratedForCombat` | after a generated card is added; after a transform | `piles.rs:add_generated_card_to_combat`, `card.rs:transform_to` | `Commands/CardPileCmd.cs:311`, `Hook.cs:251` | wired |
| `ShouldDraw / AfterPreventingDraw` | draw start; the preventer only | `piles.rs:draw` | `Commands/CardPileCmd.cs:1016-1019`, `Hook.cs:2277,1068` | wired |
| `AfterCardDrawn(Early)` | after each drawn card | `piles.rs:draw` | `Commands/CardPileCmd.cs:1063`, `Hook.cs:202` | wired |
| `AfterShuffle` | after a reshuffle | `piles.rs:shuffle` | `Commands/CardPileCmd.cs:1134`, `Hook.cs:1142` | wired |
| `AfterCardExhausted` | after a card is exhausted | `piles.rs:exhaust` | `Commands/CardCmd.cs:246`, `Hook.cs:237` | wired |
| `BeforeAttack` | attack start | `damage.rs:attack` | `Commands.Builders/AttackCommand.cs:549`, `Hook.cs:80` | wired |
| `AfterAttack` | after all hits | `damage.rs:attack` | `Commands.Builders/AttackCommand.cs:672`, `Hook.cs:92` | wired |
| `ModifyDamageAdditive / ModifyDamageMultiplicative / ModifyDamageCap` | per target of `CreatureCmd.Damage`; also intents; the card source's `EnchantDamageAdditive/Multiplicative` first | `hooks.rs:modify_damage` (from `damage.rs:damage_target`, `intent.rs`) | `Commands/CreatureCmd.cs:282`, `Hook.cs:1495-1509,2519` | wired |
| `AfterModifyingDamageAmount` | listeners that changed the damage | `damage.rs:damage_target` | `Commands/CreatureCmd.cs:283`, `Hook.cs:706` | wired |
| `BeforeDamageReceived` | before block | `damage.rs:damage_target` | `Commands/CreatureCmd.cs:284`, `Hook.cs:415` | wired |
| `ModifyHpLostBeforeOsty(+Late)` | after block, before the Osty redirect | `damage.rs:damage_target` | `Commands/CreatureCmd.cs:287`, `Hook.cs:1725` | wired |
| `AfterModifyingHpLostBeforeOsty` | listeners that changed `Truncate(x)` | `damage.rs:damage_target` | `Commands/CreatureCmd.cs:288`, `Hook.cs:766` | wired |
| `ModifyUnblockedDamageTarget` | the Osty redirect | `damage.rs:damage_target` | `Commands/CreatureCmd.cs:289`, `Hook.cs:2056` | wired |
| `ModifyHpLostAfterOsty(+Late)` | after the redirect, and again for overkill sent back | `damage.rs:damage_target` | `Commands/CreatureCmd.cs:290,304`, `Hook.cs:1725` | wired |
| `AfterModifyingHpLostAfterOsty` | listeners that changed `Truncate(x)` | `damage.rs:damage_target` | `Commands/CreatureCmd.cs:291,305`, `Hook.cs:781` | wired |
| `AfterBlockBroken` | per result whose block broke | `damage.rs:damage` | `Commands/CreatureCmd.cs:399`, `Hook.cs:107` | wired |
| `AfterCurrentHpChanged` | HP lost by a hit or kill, heal | `damage.rs:damage`, `damage.rs:kill_without_checking_win_condition`, `hp.rs:heal` | `Commands/CreatureCmd.cs:403,533,805`, `Hook.cs:386` | wired |
| `AfterDamageGiven` | per result | `damage.rs:damage` | `Commands/CreatureCmd.cs:411`, `Hook.cs:401` | wired |
| `AfterDamageReceived(+Late)` | per result whose target survived | `damage.rs:damage` | `Commands/CreatureCmd.cs:415`, `Hook.cs:429` | wired |
| `BeforeDeath` | kill, after HP is set to 0 | `damage.rs:kill_without_checking_win_condition` | `Commands/CreatureCmd.cs:535`, `Hook.cs:450` | wired |
| `ShouldDie(+Late)` | kill; a veto prevents the death | `damage.rs:kill_without_checking_win_condition` | `Commands/CreatureCmd.cs:537`, `Hook.cs:2237` | wired |
| `AfterPreventingDeath` | the ShouldDie preventer, after `AfterDeath(prevented)` | `damage.rs:kill_without_checking_win_condition` | `Commands/CreatureCmd.cs:597`, `Hook.cs:1056` | wired |
| `ShouldCreatureBeRemovedFromCombatAfterDeath` | kill | `damage.rs:kill_without_checking_win_condition` | `Commands/CreatureCmd.cs:540`, `Hook.cs:2222` | wired |
| `AfterDeath` | kill (also with `wasRemovalPrevented`) | `damage.rs:kill_without_checking_win_condition` | `Commands/CreatureCmd.cs:551,596`, `Hook.cs:462` | wired |
| `BeforeRemovedFromRoom` | monster method, a dead enemy leaving combat | `damage.rs:kill_without_checking_win_condition` | `Commands/CreatureCmd.cs:555`, `Combat/CombatManager.cs:1362-1368` | wired |
| `ShouldPowerBeRemovedAfterOwnerDeath` | power property, kill | `damage.rs:kill_without_checking_win_condition` | `Entities.Creatures/Creature.cs:673-676`, `Commands/CreatureCmd.cs:563` | wired |
| `ShouldPowerBeRemovedOnDeath` | kill, per power | `damage.rs:kill_without_checking_win_condition` | `Entities.Creatures/Creature.cs:675`, `Hook.cs:2503` | wired |
| `AfterDiedToDoom` | dispatched by `DoomPower` (`Engine::after_died_to_doom`) | none yet: `DoomPower` | `Models.Powers/DoomPower.cs:52`, `Hook.cs:496` | content call site |
| `ShouldOwnerDeathTriggerFatal` | power property read by Fatal cards (`Feed`) | none yet: Fatal cards | `Models.Cards/Feed.cs:38`, `Models/PowerModel.cs:654` | content call site |
| `ShouldStopCombatFromEnding` | whenever combat-ending is evaluated (`is_ending`) | `state.rs:is_ending` | `Combat/CombatManager.cs:417-435`, `Hook.cs:2450` | wired |
| `BeforeBlockGained` | block gain start | `hp.rs:gain_block` | `Commands/CreatureCmd.cs:676`, `Hook.cs:131` | wired |
| `ModifyBlockAdditive / ModifyBlockMultiplicative` | block gain | `hooks.rs:modify_block` (from `hp.rs:gain_block`) | `Commands/CreatureCmd.cs:678`, `Hook.cs:1322` | wired |
| `AfterModifyingBlockAmount` | listeners that changed the block (gets the `CardPlay`: `PaelsLegion`) | `hp.rs:gain_block` | `Commands/CreatureCmd.cs:680`, `Hook.cs:661` | wired |
| `AfterBlockGained` | after block gain (also 0) | `hp.rs:gain_block` | `Commands/CreatureCmd.cs:696`, `Hook.cs:143` | wired |
| `ShouldAllowHitting` | `CanReceivePowers` (apply) | `power.rs:can_receive_powers` | `Entities.Creatures/Creature.cs:309-322`, `Hook.cs:2139` | wired |
| `BeforePowerAmountChanged` | apply (new instance) or modify, before the modifiers; `PowerQuery::card` is the `cardSource` | `power.rs:apply_power_instance_from_card`, `power.rs:modify_power_amount_by` | `Commands/PowerCmd.cs:124,231`, `Hook.cs:1020` | wired |
| `ModifyPowerAmountGivenAdditive / ModifyPowerAmountGivenMultiplicative` | applier still in combat | `power.rs:modify_given` | `Commands/PowerCmd.cs:129,236`, `Hook.cs:1896` | wired |
| `TryModifyPowerAmountReceived` | apply, modify | `power.rs:apply_power`, `power.rs:modify_power_amount` | `Commands/PowerCmd.cs:131,238`, `Hook.cs:1925` | wired |
| `BeforeApplied` | power method, before a new instance is added | `power.rs:apply_power` | `Commands/PowerCmd.cs:136` | wired |
| `AfterModifyingPowerAmountGiven / AfterModifyingPowerAmountReceived` | listeners that changed the amount | `power.rs:apply_power`, `power.rs:modify_power_amount` | `Commands/PowerCmd.cs:154,156,244,246`, `Hook.cs:811,826` | wired |
| `AfterApplied` | power method, after a new non-zero instance | `power.rs:apply_power` | `Commands/PowerCmd.cs:159` | wired |
| `AfterPowerAmountChanged` | after a non-zero change | `power.rs:apply_power`, `power.rs:modify_power_amount` | `Commands/PowerCmd.cs:160,249`, `Hook.cs:1032` | wired |
| `ModifyOrbValue` | whenever an orb's `PassiveVal`/`EvokeVal` is read (fold, guarded) | `orb.rs:orb_passive_val`, `orb.rs:orb_evoke_val` (`Hooked::modify_orb_value`) | `Models/OrbModel.cs:ModifyOrbValue`, `Hook.cs:1882` | wired |
| `ModifyOrbPassiveTriggerCounts / AfterModifyingOrbPassiveTriggerCount` | `OrbModel.TriggerPassive`, before the passives (the after hook to the changers) | `orb.rs:trigger_orb_passive` | `Models/OrbModel.cs:246-262`, `Hook.cs:1863,796` | wired (no in-scope listener) |
| `AfterOrbChanneled` | after an orb joined the queue and was logged (`History.OrbChanneled`) | `orb.rs:channel` | `Commands/OrbCmd.cs:68-92`, `Hook.cs:856` | wired (no in-scope listener) |
| `AfterOrbEvoked` | after an orb's `Evoke`, while the owner is in combat | `orb.rs:evoke` | `Commands/OrbCmd.cs:125-153`, `Hook.cs:870` | wired (no in-scope listener) |
| `AfterRemoved` | power method, after removal (also after its owner's death); the instance is off its owner but keeps its fields, passed as `power` (C23) | `power.rs:remove_power`, `damage.rs:kill_without_checking_win_condition` | `Commands/PowerCmd.cs:297`, `Commands/CreatureCmd.cs:566` | wired |
| `BeforePotionUsed / AfterPotionUsed` | potion use | `potion_use.rs:use_potion_action` | `Models/PotionModel.cs:297-354`, `Hook.cs:996,1008` | wired |
| `AfterPotionProcured` | `PotionCmd.TryToProcure` (`Engine::try_to_procure_potion`) | `potion.rs:try_to_procure_potion` | `Commands/PotionCmd.cs:46`, `Hook.cs:984` | wired (no in-scope caller) |
| `AfterPotionDiscarded` | `PotionCmd.Discard` (`Engine::discard_potion`) | `potion.rs:discard_potion` | `Commands/PotionCmd.cs:59`, `Hook.cs:972` | wired (no in-scope caller) |
| `ModifyXValue` | `ResolveEnergyXValue` of an X-cost card | `hooks.rs:resolve_energy_x_value` (from `cards/eradicate.rs`) | `Models/CardModel.cs:1123`, `Hook.cs:2079` | wired |
| `AfterDowngraded` | card method, end of `DowngradeInternal` | `card.rs:downgrade` | `Models/CardModel.cs:2142-2156` | wired (no in-scope caller) |
| `AfterCombatEnd` | win only, before the player's teardown | `turn.rs:check_win_condition` | `Combat/CombatManager.cs:1315`, `Hook.cs:328` | wired |
| `AfterCombatVictory(Early)` | win only, after the player's teardown | `turn.rs:check_win_condition` | `Combat/CombatManager.cs:1326`, `Hook.cs:352` | wired |
| `AfterObtained` | relic joins the run (`RelicCmd.Obtain`; also `Replace`, at the old index) | `run/ctx.rs:obtain` -> `RelicModel::after_obtained` | `Commands/RelicCmd.cs:35-55` | wired |
| `AfterRoomEntered` (no fight) | rest site (after its options), treasure (before the chest), event (after `BeginEvent`) | `run/hooks.rs:after_room_entered` (from `run/mod.rs`, `run/events.rs:begin_event`) | `Rooms/RestSiteRoom.cs:37`, `Rooms/TreasureRoom.cs:44`, `Rooms/EventRoom.cs:85` | wired (`RelicModel::after_room_entered_out_of_combat`; Neow's room has no listener) |
| `AfterCombatEnd` (deck half) | after a won fight, before its rewards: listeners that act on the deck | `run/mod.rs:win_combat` -> `RelicModel::after_combat_end_on_deck` | `Hook.cs:328` | wired (`FishingRod`) |
| `ModifyGoldGained / AfterModifyingGoldGained / AfterGoldGained` | `PlayerCmd.GainGold` in combat (`AfterGoldGained`: relics and potions only); outside combat every gold gain (rewards, treasure, events, relics) | `gold.rs:gain_gold`; `run/state.rs:gain_gold` -> `run/hooks.rs` | `Commands/PlayerCmd.cs:143,144,176`, `Hook.cs:1634,736,481` | wired (run: `*_out_of_combat`; combat: no in-scope caller) |
| `AfterCardChangedPiles` (deck) | a card joined the deck (`CardPileCmd.Add` to the deck, after the batch; a deck transform's replacement) | `run/state.rs:add_card`, `transform_to`, `transform_many_to_random` -> `run/hooks.rs:after_card_added_to_deck` | `Commands/CardPileCmd.cs:540`, `Commands/CardCmd.cs:451`, `Hook.cs:167` | wired (`LuckyFysh`) |
| `BeforeCardRemoved` | a card leaves the deck (`RemoveFromDeck`; not a transform) | `run/state.rs:remove_card` | `Commands/CardPileCmd.cs:62`, `Hook.cs:299` | wired |
| `TryModifyCardBeingAddedToDeck(+Late)` | a card joins the deck (rewards, events, relics, a transform's replacement) | `run/state.rs:add_card`, `transform_to` | `Commands/CardPileCmd.cs:496`, `Commands/CardCmd.cs:434`, `Hook.cs:1357` | wired |
| `TryModifyCardRewardOptions(+Late) / AfterModifyingCardRewardOptions` | end of `CardFactory.CreateForReward` | `run/rewards.rs:create_card_reward` -> `run/hooks.rs:modify_card_reward_options` | `Factories/CardFactory.cs:104`, `Hook.cs:1454,691` | wired |
| `ModifyCardRewardCreationOptions(+Late)` | per card of `CreateForReward` | `run/rewards.rs:create_card_reward` | `Factories/CardFactory.cs:216`, `Hook.cs:1438` | wired |
| `BeforeCombatRewardOffered` | a won fight's rewards, after generation | `run/rewards.rs:generate_combat_rewards` | `Rooms/CombatRoom.cs:268`, `Hook.cs:340` | wired |
| `TryModifyRewards(+Late) / AfterModifyingRewards` | `GenerateWithoutOffering`: after the populate pass (added rewards populated after every listener); custom rewards with no room | `run/rewards.rs:generate_combat_rewards`, `custom_rewards` | `Rewards/RewardsSet.cs:125-147`, `Hook.cs:1989,841` | wired |
| `ShouldForcePotionReward` | combat rewards, before the potion roll (a forced potion skips the draw) | `run/rewards.rs:generate_combat_rewards` | `Odds/PotionRewardOdds.cs:54-72`, `Hook.cs:2480` | wired |
| `ModifyMerchantPrice` | every merchant `Cost` read in a `MerchantRoom` (`(int)` of the folded decimal) | `run/hooks.rs:modify_merchant_price` via `run/shop.rs:price` | `Entities.Merchant/MerchantEntry.cs:19-30`, `Hook.cs:1826` | wired |
| `ShouldRefillMerchantEntry` | after a successful purchase: restock, else clear | `run/mod.rs:shop_action` | `Entities.Merchant/MerchantEntry.cs:69-96`, `Hook.cs:2431` | wired |
| `AfterRoomEntered` (shop) | after the inventory is generated | `run/mod.rs:enter_node` | `Rooms/MerchantRoom.cs:52-70` | wired |
| `ModifyMerchantCardCreationResults / ModifyMerchantCardPool / ModifyMerchantCardRarity / ModifyCardRewardUpgradeOdds` | shop card generation | none (no ported listener: the eggs, `FresnelLens`, the `CharacterCards` modifier) | `Factories/CardFactory.cs:40-80`, `Hook.cs:1791` | run layer (no listener) |
| `AfterItemPurchased` | after a shop card, relic or potion purchase and its restock (or clear); after a shop removal's pick | `run/mod.rs:shop_action`, `run/mod.rs` (`SelectPurpose::ShopRemoval`) -> `run/hooks.rs:after_item_purchased` | `Entities.Merchant/MerchantEntry.cs:92`, `MerchantCardRemovalEntry.cs:49`, `Hook.cs:637` | wired (C23, `MawBank`; `LordsParasol`'s free purchases are unported) |
| `TryModifyRestSiteOptions / ShouldDisableRemainingRestSiteOptions` | rest-site options; after an option succeeds | `run/mod.rs:rest_options`, `after_rest_option` | `Hook.cs:1957,2262`, `Multiplayer.Game/RestSiteSynchronizer.cs:199-218` | wired |
| `ModifyRestSiteHealAmount / AfterRestSiteHeal / AfterRestSiteSmith` | rest-site heal and Smith | `run/mod.rs:rest_heal`, `resolve_deck_purpose` | `Entities.RestSite/HealRestSiteOption.cs:60-113`, `SmithRestSiteOption.cs:72`, `Hook.cs:1944,1080,1092` | wired |
| `ModifyExtraRestSiteHealText / TryModifyRestSiteHealRewards` | rest-site heal text and rewards | none: display only / no in-scope listener | `Hook.cs:1654,1973` | run layer (not needed) |
| `ShouldGenerateTreasure` | a treasure room, after `AfterRoomEntered` | `run/mod.rs:enter_treasure` | `Multiplayer.Game/OneOffSynchronizer.cs:130`, `Hook.cs:2324` | wired |
| `ModifyUnknownMapPointRoomTypes` | a `?` roll, after the blacklist | `run/mod.rs:enter_node` -> `odds.rs:UnknownMapPointOdds::roll` | `Odds/UnknownMapPointOdds.cs:138`, `Hook.cs:2069` | wired |
| `ModifyGeneratedMap(+Late)` | `RunManager.GenerateMap`: entering acts 2-3, `RunCtx::regenerate_map` (none at run start: no listener exists yet) | `run/state.rs:RunState::generate_map` -> `run/hooks.rs:modify_generated_map` (deck `SpoilsMap`, then relics; then the late pass) | `Hook.cs:1666,1679`, `Runs/RunManager.cs:799-831` | wired |
| `AfterMapGenerated` | after `ModifyGeneratedMap` | none: only quest markers (`SpoilsMap`, `FurCoat` marks its own data) | `Hook.cs:649` | not modelled |
| `ModifyNextEvent` | `PullNextEvent`, before `AddVisitedEvent` | `run/events.rs:pull_next_event` | `Models/ActModel.cs:440`, `Hook.cs:1838` | wired |
| `BeforeEventStarted` / `OnEventFinished` / `OnRoomEnter` | event model methods | `EventModel::begin` (`BeforeEventStarted`); the others have no in-scope override | `Models/EventModel.cs:458,488,505` | wired / no listener |
| `OnEnchant` | enchantment method, `CardCmd.Enchant` (`ModifyCard`) | none: no in-scope enchantment overrides it | `Models/EnchantmentModel.cs:355-395` | run layer |
