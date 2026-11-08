//! Hook listeners and dispatch (`Hook.cs`, `CombatState.IterateHookListeners`).
//!
//! Every model (power, monster, relic, potion, card) implements [`Hooks`]; each method defaults
//! to "does nothing" (or the neutral value), so a model only overrides the hooks its C# class
//! overrides. The engine calls the typed dispatchers below at the game's call sites; the table
//! in `docs/specs/hooks.md` lists every hook with its call site.
//!
//! Dispatch goes through each kind's `model()` table (`&'static dyn` models; one dynamic call
//! per listener, so the code per call site does not grow with the content). `#[sts2_macros::hooks]`
//! on a model's `impl Hooks` records the [`Hook`]s it overrides (its `HOOKS`, gathered per kind
//! into a table by `build.rs`: `CardKind::hooks` and so on), and dispatch only visits those
//! listeners: every default is a no-op or the neutral value, so skipping it is
//! exact. The state keeps the union of the listeners' hooks, and a pass walks the state in
//! listener order ([`super::listeners`]).
//!
//! Async hooks walk a snapshot of the pass's listeners, re-checking each entry with
//! [`CombatState::contains_listener`] just before calling it, so listeners removed mid-dispatch
//! are skipped and listeners added mid-dispatch are not visited (`CombatState.cs:411-486`).
//! Guarded hooks (`IterateCombatHookListeners`) see no listeners when combat is over or ending
//! at the moment the dispatch starts, except while combat is starting (`Hook.cs:53-63`). A hook
//! with several passes (`XEarly`, `X`, `XLate`) runs a full listener pass per method. The
//! `AfterModifyingX` hooks only reach the listeners that changed the value (`Hook.cs:661-851`).
//!
//! The fork-join turn-boundary hooks (`BeforeSideTurnStart`, `BeforeSideTurnEnd`,
//! `AfterSideTurnEnd`, `BeforeFlush`, `AfterDeath`, `AfterDiedToDoom`) run listeners
//! sequentially; that is exact as long as no listener raises a player choice (no in-scope
//! content does, `docs/specs/combat-loop.md` §2.4).

use super::card_play::{CardPlay, Resources, ResultLocation};
use super::commands::damage::{Attack, DamageResult};
use super::engine::{Engine, Flow, ValueProps};
use super::listeners::Listeners;
use super::state::{
    CardId, CombatState, CreatureId, Creatures, Orb, PileType, Potion, PotionId, Power, PowerId,
    RelicId, Side,
};
use crate::combat::AutoPlayType;
use crate::content::cards::{CardKeyword, Keywords};
use crate::content::potions::PotionKind;
use crate::content::powers::{PowerData, PowerKind};
use crate::content::relics::RelicKind;
use crate::decimal::Decimal;

/// A hook listener (`AbstractModel`) by id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Listener {
    Power {
        owner: CreatureId,
        power: PowerId,
    },
    Monster(CreatureId),
    Relic(RelicId),
    Potion(PotionId),
    Card(CardId),
    /// The affliction on a card.
    Affliction(CardId),
    /// The enchantment on a card.
    Enchantment(CardId),
}

/// A power as a listener: its owner and instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PowerRef {
    pub owner: CreatureId,
    pub id: PowerId,
}

/// Whether a dispatch honours the combat-ending guard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Guard {
    /// `IterateCombatHookListeners`: silent once combat is over or ending (not while starting).
    Combat,
    /// `combatState.IterateHookListeners()` or run-level listeners: always runs.
    None,
}

/// The arguments of the `ModifyDamage*` hooks.
#[derive(Clone, Copy, Debug)]
pub struct DamageQuery {
    pub target: Option<CreatureId>,
    pub dealer: Option<CreatureId>,
    pub props: ValueProps,
    pub card: Option<CardId>,
}

/// The arguments of the `ModifyHpLost*` hooks.
#[derive(Clone, Copy, Debug)]
pub struct HpLossQuery {
    pub target: CreatureId,
    pub props: ValueProps,
    pub dealer: Option<CreatureId>,
    pub card: Option<CardId>,
}

/// The arguments of the `ModifyBlock*` and `Before/AfterBlockGained` hooks. `card` is
/// `cardPlay?.Card`; `play` is the `cardPlay` (`ModifyBlock*` only).
#[derive(Clone, Copy, Debug)]
pub struct BlockQuery {
    pub target: CreatureId,
    pub props: ValueProps,
    pub card: Option<CardId>,
    pub play: Option<CardPlay>,
}

/// One damage result as `AfterDamageGiven` and `AfterDamageReceived(Late)` see it.
#[derive(Clone, Copy, Debug)]
pub struct DamageEvent {
    /// The creature that took the hit (`result.receiver`).
    pub target: CreatureId,
    pub result: DamageResult,
    pub props: ValueProps,
    pub dealer: Option<CreatureId>,
    pub card: Option<CardId>,
}

/// A power about to be applied or changed, as `BeforePowerAmountChanged`, the
/// `ModifyPowerAmount*` hooks and `AfterModifyingPowerAmount*` see it. A newly applied power is
/// not on its target yet, so it is identified by kind; `applier` is the giver and `card` the
/// `cardSource`.
#[derive(Clone, Copy, Debug)]
pub struct PowerQuery {
    pub kind: PowerKind,
    pub target: CreatureId,
    pub applier: Option<CreatureId>,
    pub card: Option<CardId>,
}

/// Which `ModifyHpLost` passes to run (`HpLossHookPhase`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HpLossPhase {
    BeforeOsty,
    AfterOsty,
}

/// The listener hooks of `AbstractModel`, one method per hook pass. `Me` identifies the
/// listening model instance. Async hooks get the [`Engine`] and return [`Flow`]; value hooks
/// (`Modify*`, `Should*`, `TryModify*`) only read the state.
///
/// Every default does nothing (or returns the neutral value), like the C# base class. Hooks
/// that only fire outside combat are relic methods dispatched by the run layer
/// ([`crate::content::relics::RelicModel`], `run/hooks.rs`).
#[expect(
    unused_variables,
    reason = "default bodies ignore their arguments, like the C# virtual methods"
)]
#[sts2_macros::hook_ids]
pub trait Hooks {
    /// The listener's id.
    type Me: Copy;

    // Combat start and the player turn start.

    /// `AfterRoomEntered` (run-level, combat not started yet; deck cards, relics, potions).
    fn after_room_entered(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }
    /// `BeforeCombatStart`.
    fn before_combat_start(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }
    /// `BeforeCombatStartLate`.
    fn before_combat_start_late(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }
    /// `BeforeSideTurnStart(side, participants)`.
    fn before_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        Ok(())
    }
    /// `AfterBlockCleared(creature)`: once per creature starting its turn, even if its block
    /// was kept.
    fn after_block_cleared(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        creature: CreatureId,
    ) -> Flow {
        Ok(())
    }
    /// `AfterPreventingBlockClear(preventer, creature)`: only the preventer (`Me`) is called.
    fn after_preventing_block_clear(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        creature: CreatureId,
    ) -> Flow {
        Ok(())
    }
    /// `AfterEnergyReset` (the only player is implied).
    fn after_energy_reset(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }
    /// `AfterEnergyResetLate`.
    fn after_energy_reset_late(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }
    /// `BeforeHandDraw`.
    fn before_hand_draw(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }
    /// `BeforeHandDrawLate`.
    fn before_hand_draw_late(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }
    /// `AfterModifyingHandDraw`: only listeners whose `ModifyHandDraw(Late)` changed `(int)`.
    fn after_modifying_hand_draw(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }
    /// `AfterPlayerTurnStartEarly`.
    fn after_player_turn_start_early(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }
    /// `AfterPlayerTurnStart`.
    fn after_player_turn_start(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }
    /// `AfterPlayerTurnStartLate`.
    fn after_player_turn_start_late(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }
    /// `AfterSideTurnStart(side, participants)`.
    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        Ok(())
    }
    /// `AfterSideTurnStartLate(side, participants)`.
    fn after_side_turn_start_late(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        Ok(())
    }
    /// `AfterAutoPrePlayPhaseEnteredEarly`.
    fn after_auto_pre_play_phase_entered_early(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }
    /// `AfterAutoPrePlayPhaseEntered`.
    fn after_auto_pre_play_phase_entered(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }
    /// `AfterAutoPrePlayPhaseEnteredLate`.
    fn after_auto_pre_play_phase_entered_late(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }
    /// `AfterHandEmptied`.
    fn after_hand_emptied(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }

    // Turn end.

    /// `AfterAutoPostPlayPhaseEntered`.
    fn after_auto_post_play_phase_entered(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }
    /// `BeforeSideTurnEndVeryEarly(side, participants)`.
    fn before_side_turn_end_very_early(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        Ok(())
    }
    /// `BeforeSideTurnEndEarly(side, participants)`.
    fn before_side_turn_end_early(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        Ok(())
    }
    /// `BeforeSideTurnEnd(side, participants)`.
    fn before_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        Ok(())
    }
    /// `BeforeFlush`.
    fn before_flush(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }
    /// `BeforeFlushLate`.
    fn before_flush_late(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }
    /// `AfterTakingExtraTurn(player)` (the only player).
    fn after_taking_extra_turn(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }
    /// `AfterFlush(flushedCards, retainedCards)`.
    fn after_flush(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        flushed: &[CardId],
        retained: &[CardId],
    ) -> Flow {
        Ok(())
    }
    /// `AfterSideTurnEnd(side, participants)`.
    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        Ok(())
    }
    /// `AfterSideTurnEndLate(side, participants)`.
    fn after_side_turn_end_late(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        Ok(())
    }

    // Stars and forging.

    /// `AfterStarsGained(amount, gainer)` (the only player).
    fn after_stars_gained(&self, eng: &mut Engine<'_>, me: Self::Me, amount: i32) -> Flow {
        Ok(())
    }
    /// `AfterStarsSpent(amount, spender)`: a play paid `amount > 0` stars (the only player).
    fn after_stars_spent(&self, eng: &mut Engine<'_>, me: Self::Me, amount: i32) -> Flow {
        Ok(())
    }
    /// `AfterForge(amount, forger, source)` (the only player); `source` is the model that
    /// forged, if any.
    fn after_forge(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        amount: Decimal,
        source: Option<Listener>,
    ) -> Flow {
        Ok(())
    }

    // Card play and card piles.

    /// `AfterEnergySpent(card, amount)`: also for 0.
    fn after_energy_spent(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        card: CardId,
        amount: i32,
    ) -> Flow {
        Ok(())
    }
    /// `AfterModifyingCardPlayResultLocation(card, location)`: only listeners whose
    /// `ModifyCardPlayResultLocation` changed it; `location` is the final one.
    fn after_modifying_card_play_result_location(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        card: CardId,
        location: ResultLocation,
    ) -> Flow {
        Ok(())
    }
    /// `AfterModifyingCardPlayCount(card)`: only listeners whose `ModifyCardPlayCount` changed it.
    fn after_modifying_card_play_count(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        card: CardId,
    ) -> Flow {
        Ok(())
    }
    /// `BeforeCardPlayed(cardPlay)`.
    fn before_card_played(&self, eng: &mut Engine<'_>, me: Self::Me, play: &CardPlay) -> Flow {
        Ok(())
    }
    /// `AfterCardPlayed(cardPlay)`.
    fn after_card_played(&self, eng: &mut Engine<'_>, me: Self::Me, play: &CardPlay) -> Flow {
        Ok(())
    }
    /// `AfterCardPlayedLate(cardPlay)`.
    fn after_card_played_late(&self, eng: &mut Engine<'_>, me: Self::Me, play: &CardPlay) -> Flow {
        Ok(())
    }
    /// `AfterCreatureAddedToCombat(creature)`: a creature added mid-combat (`CreatureCmd.Add`).
    fn after_creature_added_to_combat(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        creature: CreatureId,
    ) -> Flow {
        Ok(())
    }
    /// `AfterCardDiscarded(card)`: after `CardCmd.Discard` moved the card.
    fn after_card_discarded(&self, eng: &mut Engine<'_>, me: Self::Me, card: CardId) -> Flow {
        Ok(())
    }
    /// `AfterCardEnteredCombat(card)`: a card added to a combat pile from no pile.
    fn after_card_entered_combat(&self, eng: &mut Engine<'_>, me: Self::Me, card: CardId) -> Flow {
        Ok(())
    }
    /// `AfterCardChangedPiles(card, oldPileType)`; `None` is `PileType.None`.
    fn after_card_changed_piles(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        card: CardId,
        old_pile: Option<PileType>,
    ) -> Flow {
        Ok(())
    }
    /// `AfterCardChangedPilesLate(card, oldPileType)`.
    fn after_card_changed_piles_late(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        card: CardId,
        old_pile: Option<PileType>,
    ) -> Flow {
        Ok(())
    }
    /// `AfterCardGeneratedForCombat(card, creator)`: `creator` is the creating player's
    /// creature, if a player created it.
    fn after_card_generated_for_combat(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        card: CardId,
        creator: Option<CreatureId>,
    ) -> Flow {
        Ok(())
    }
    /// `AfterCardDrawnEarly(card, fromHandDraw)`.
    fn after_card_drawn_early(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        card: CardId,
        from_hand_draw: bool,
    ) -> Flow {
        Ok(())
    }
    /// `AfterCardDrawn(card, fromHandDraw)`.
    fn after_card_drawn(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        card: CardId,
        from_hand_draw: bool,
    ) -> Flow {
        Ok(())
    }
    /// `AfterPreventingDraw`: only the `ShouldDraw` preventer (`Me`) is called.
    fn after_preventing_draw(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }
    /// `AfterShuffle`.
    fn after_shuffle(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }
    /// `AfterCardExhausted(card, causedByEthereal)`.
    fn after_card_exhausted(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        card: CardId,
        caused_by_ethereal: bool,
    ) -> Flow {
        Ok(())
    }

    // Attacks, damage, HP and death.

    /// `BeforeAttack(command)`.
    fn before_attack(&self, eng: &mut Engine<'_>, me: Self::Me, attack: &Attack) -> Flow {
        Ok(())
    }
    /// `AfterAttack(command)`; `results` is `command.Results`, every hit in order.
    fn after_attack(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        attack: &Attack,
        results: &[DamageResult],
    ) -> Flow {
        Ok(())
    }
    /// `AfterModifyingDamageAmount(cardSource)`: only listeners that changed the damage.
    fn after_modifying_damage_amount(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        card: Option<CardId>,
    ) -> Flow {
        Ok(())
    }
    /// `BeforeDamageReceived(target, amount, props, dealer, cardSource)`; `amount` is the
    /// modified damage, before block.
    fn before_damage_received(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        query: HpLossQuery,
        amount: Decimal,
    ) -> Flow {
        Ok(())
    }
    /// `AfterModifyingHpLostBeforeOsty`: only listeners that changed `Truncate(x)`.
    fn after_modifying_hp_lost_before_osty(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }
    /// `AfterModifyingHpLostAfterOsty`: only listeners that changed `Truncate(x)`.
    fn after_modifying_hp_lost_after_osty(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }
    /// `AfterBlockBroken(target, breaker)`.
    fn after_block_broken(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        target: CreatureId,
        breaker: Option<CreatureId>,
    ) -> Flow {
        Ok(())
    }
    /// `AfterCurrentHpChanged(creature, delta)`.
    fn after_current_hp_changed(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        creature: CreatureId,
        delta: Decimal,
    ) -> Flow {
        Ok(())
    }
    /// `AfterDamageGiven(dealer, result, props, target, cardSource)`.
    fn after_damage_given(&self, eng: &mut Engine<'_>, me: Self::Me, hit: DamageEvent) -> Flow {
        Ok(())
    }
    /// `AfterDamageReceived(target, result, props, dealer, cardSource)`: not for a hit that
    /// killed its target.
    fn after_damage_received(&self, eng: &mut Engine<'_>, me: Self::Me, hit: DamageEvent) -> Flow {
        Ok(())
    }
    /// `AfterDamageReceivedLate`.
    fn after_damage_received_late(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        hit: DamageEvent,
    ) -> Flow {
        Ok(())
    }
    /// `BeforeDeath(creature)`: its HP is already 0.
    fn before_death(&self, eng: &mut Engine<'_>, me: Self::Me, creature: CreatureId) -> Flow {
        Ok(())
    }
    /// `AfterDeath(creature, wasRemovalPrevented)`.
    fn after_death(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        creature: CreatureId,
        was_removal_prevented: bool,
    ) -> Flow {
        Ok(())
    }
    /// `AfterPreventingDeath(creature)`: only the `ShouldDie` preventer (`Me`) is called.
    fn after_preventing_death(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        creature: CreatureId,
    ) -> Flow {
        Ok(())
    }
    /// `AfterDiedToDoom(creatures)`, dispatched by `DoomPower` itself.
    fn after_died_to_doom(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        creatures: &[CreatureId],
    ) -> Flow {
        Ok(())
    }

    // Block and powers.

    /// `BeforeBlockGained(creature, amount, props, cardSource)`.
    fn before_block_gained(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        query: BlockQuery,
        amount: Decimal,
    ) -> Flow {
        Ok(())
    }
    /// `AfterModifyingBlockAmount(modifiedAmount, cardSource, cardPlay)`: only listeners that
    /// changed it.
    fn after_modifying_block_amount(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        amount: Decimal,
        card: Option<CardId>,
        play: Option<&CardPlay>,
    ) -> Flow {
        Ok(())
    }
    /// `AfterBlockGained(creature, amount, props, cardSource)`: also when nothing was gained.
    fn after_block_gained(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        query: BlockQuery,
        amount: Decimal,
    ) -> Flow {
        Ok(())
    }
    /// `BeforePowerAmountChanged(power, amount, target, applier)`, before the modifiers.
    fn before_power_amount_changed(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        power: PowerQuery,
        amount: Decimal,
    ) -> Flow {
        Ok(())
    }
    /// `AfterModifyingPowerAmountGiven(power)`: only listeners that changed it.
    fn after_modifying_power_amount_given(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        power: PowerQuery,
    ) -> Flow {
        Ok(())
    }
    /// `AfterModifyingPowerAmountReceived(power)`: only listeners that changed it.
    fn after_modifying_power_amount_received(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        power: PowerQuery,
    ) -> Flow {
        Ok(())
    }
    /// `AfterPowerAmountChanged(power, amount, applier)`: `amount` is the applied change.
    fn after_power_amount_changed(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        power: PowerRef,
        amount: Decimal,
        applier: Option<CreatureId>,
    ) -> Flow {
        Ok(())
    }

    // Orbs.

    /// `AfterModifyingOrbPassiveTriggerCount(orb)`: the listeners that changed the count.
    fn after_modifying_orb_passive_trigger_count(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        orb: Orb,
    ) -> Flow {
        Ok(())
    }
    /// `AfterOrbChanneled(player, orb)`, for the only player.
    fn after_orb_channeled(&self, eng: &mut Engine<'_>, me: Self::Me, orb: Orb) -> Flow {
        Ok(())
    }
    /// `AfterOrbEvoked(orb, targets)`: the creatures the evoke affected.
    fn after_orb_evoked(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        orb: Orb,
        targets: &[CreatureId],
    ) -> Flow {
        Ok(())
    }

    // Potions and combat end.

    /// `BeforePotionUsed(potion, target)`; the potion has already left its slot.
    fn before_potion_used(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        potion: Potion,
        target: Option<CreatureId>,
    ) -> Flow {
        Ok(())
    }
    /// `AfterPotionUsed(potion, target)`.
    fn after_potion_used(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        potion: Potion,
        target: Option<CreatureId>,
    ) -> Flow {
        Ok(())
    }
    /// `AfterPotionProcured(potion)`.
    fn after_potion_procured(&self, eng: &mut Engine<'_>, me: Self::Me, potion: Potion) -> Flow {
        Ok(())
    }
    /// `AfterPotionDiscarded(potion)`.
    fn after_potion_discarded(&self, eng: &mut Engine<'_>, me: Self::Me, potion: Potion) -> Flow {
        Ok(())
    }
    /// `AfterModifyingGoldGained(player, amount)`: the listeners that changed `(int)` gold.
    fn after_modifying_gold_gained(
        &self,
        eng: &mut Engine<'_>,
        me: Self::Me,
        amount: Decimal,
    ) -> Flow {
        Ok(())
    }
    /// `AfterGoldGained(player)`: run-level listeners only (relics, potions).
    fn after_gold_gained(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }
    /// `AfterCombatEnd` (won combats only; listeners still see combat powers and piles).
    fn after_combat_end(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }
    /// `AfterCombatVictoryEarly`: after the player's combat teardown (no powers, empty piles).
    fn after_combat_victory_early(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }
    /// `AfterCombatVictory`.
    fn after_combat_victory(&self, eng: &mut Engine<'_>, me: Self::Me) -> Flow {
        Ok(())
    }

    // Value hooks.

    /// `ModifyDamageAdditive`: returns the amount to add; `amount` is the running value.
    fn modify_damage_additive(
        &self,
        s: &CombatState,
        me: Self::Me,
        query: DamageQuery,
        amount: Decimal,
    ) -> Decimal {
        Decimal::ZERO
    }
    /// `ModifyDamageMultiplicative`: returns the factor; `amount` is the running value.
    fn modify_damage_multiplicative(
        &self,
        s: &CombatState,
        me: Self::Me,
        query: DamageQuery,
        amount: Decimal,
    ) -> Decimal {
        Decimal::ONE
    }
    /// `ModifyDamageCap`: returns the cap (`decimal.MaxValue` for none).
    fn modify_damage_cap(&self, s: &CombatState, me: Self::Me, query: DamageQuery) -> Decimal {
        Decimal::MAX
    }
    /// `ModifyHpLostBeforeOsty`: returns the new running value.
    fn modify_hp_lost_before_osty(
        &self,
        s: &CombatState,
        me: Self::Me,
        query: HpLossQuery,
        amount: Decimal,
    ) -> Decimal {
        amount
    }
    /// `ModifyHpLostBeforeOstyLate`.
    fn modify_hp_lost_before_osty_late(
        &self,
        s: &CombatState,
        me: Self::Me,
        query: HpLossQuery,
        amount: Decimal,
    ) -> Decimal {
        amount
    }
    /// `ModifyHpLostAfterOsty`: returns the new running value.
    fn modify_hp_lost_after_osty(
        &self,
        s: &CombatState,
        me: Self::Me,
        query: HpLossQuery,
        amount: Decimal,
    ) -> Decimal {
        amount
    }
    /// `ModifyHpLostAfterOstyLate`.
    fn modify_hp_lost_after_osty_late(
        &self,
        s: &CombatState,
        me: Self::Me,
        query: HpLossQuery,
        amount: Decimal,
    ) -> Decimal {
        amount
    }
    /// `ModifyUnblockedDamageTarget(target, amount, props, dealer)`: returns the (possibly
    /// redirected) target.
    fn modify_unblocked_damage_target(
        &self,
        s: &CombatState,
        me: Self::Me,
        target: CreatureId,
        amount: Decimal,
        props: ValueProps,
        dealer: Option<CreatureId>,
    ) -> CreatureId {
        target
    }
    /// `ModifyBlockAdditive`: returns the amount to add; `amount` is the running value.
    fn modify_block_additive(
        &self,
        s: &CombatState,
        me: Self::Me,
        query: BlockQuery,
        amount: Decimal,
    ) -> Decimal {
        Decimal::ZERO
    }
    /// `ModifyBlockMultiplicative`: returns the factor; `amount` is the running value.
    fn modify_block_multiplicative(
        &self,
        s: &CombatState,
        me: Self::Me,
        query: BlockQuery,
        amount: Decimal,
    ) -> Decimal {
        Decimal::ONE
    }
    /// `ModifyShuffleOrder(player, cards, isInitialShuffle)`: reorders `cards`, the shuffled
    /// draw pile (index 0 is the top).
    fn modify_shuffle_order(
        &self,
        s: &CombatState,
        me: Self::Me,
        cards: &mut Vec<CardId>,
        is_initial: bool,
    ) {
    }
    /// `ModifyHandDraw`: returns the new running count.
    fn modify_hand_draw(&self, s: &CombatState, me: Self::Me, count: Decimal) -> Decimal {
        count
    }
    /// `ModifyHandDrawLate`.
    fn modify_hand_draw_late(&self, s: &CombatState, me: Self::Me, count: Decimal) -> Decimal {
        count
    }
    /// `ModifyMaxEnergy`: returns the new running value.
    fn modify_max_energy(&self, s: &CombatState, me: Self::Me, amount: Decimal) -> Decimal {
        amount
    }
    /// `ModifyCardPlayResultLocation(card, isAutoPlay, resources, location)`: returns the new
    /// location.
    fn modify_card_play_result_location(
        &self,
        s: &CombatState,
        me: Self::Me,
        card: CardId,
        is_auto_play: bool,
        resources: Resources,
        location: ResultLocation,
    ) -> ResultLocation {
        location
    }
    /// `ModifyCardPlayCount(card, target, playCount)`: returns the new count.
    fn modify_card_play_count(
        &self,
        s: &CombatState,
        me: Self::Me,
        card: CardId,
        target: Option<CreatureId>,
        count: u32,
    ) -> u32 {
        count
    }
    /// `TryModifyEnergyCostInCombat(card, cost)`: `Some(new cost)` if it changes it.
    fn try_modify_energy_cost_in_combat(
        &self,
        s: &CombatState,
        me: Self::Me,
        card: CardId,
        cost: Decimal,
    ) -> Option<Decimal> {
        None
    }
    /// `TryModifyEnergyCostInCombatLate(card, cost)`.
    fn try_modify_energy_cost_in_combat_late(
        &self,
        s: &CombatState,
        me: Self::Me,
        card: CardId,
        cost: Decimal,
    ) -> Option<Decimal> {
        None
    }
    /// `TryModifyStarCost(card, cost)`: `Some(new cost)` if it changes it.
    fn try_modify_star_cost(
        &self,
        s: &CombatState,
        me: Self::Me,
        card: CardId,
        cost: Decimal,
    ) -> Option<Decimal> {
        None
    }
    /// `TryModifyKeywordsInCombat(card, keywords)` (`AbstractModel.cs:2066`): add or remove
    /// global keywords of `card` in place; returns whether it changed the set (unused by the
    /// hook, as in C#).
    fn try_modify_keywords_in_combat(
        &self,
        s: &CombatState,
        me: Self::Me,
        card: CardId,
        keywords: &mut Keywords,
    ) -> bool {
        false
    }
    /// `ShouldGainStars(amount, player)` (the only player).
    fn should_gain_stars(&self, s: &CombatState, me: Self::Me, amount: Decimal) -> bool {
        true
    }
    /// `ModifyXValue(card, value)`: the X of an X-cost card (`ResolveEnergyXValue`).
    fn modify_x_value(&self, s: &CombatState, me: Self::Me, card: CardId, value: i32) -> i32 {
        value
    }
    /// `ModifyPowerAmountGivenAdditive`: returns the amount to add.
    fn modify_power_amount_given_additive(
        &self,
        s: &CombatState,
        me: Self::Me,
        power: PowerQuery,
        amount: Decimal,
    ) -> Decimal {
        Decimal::ZERO
    }
    /// `ModifyPowerAmountGivenMultiplicative`: returns the factor.
    fn modify_power_amount_given_multiplicative(
        &self,
        s: &CombatState,
        me: Self::Me,
        power: PowerQuery,
        amount: Decimal,
    ) -> Decimal {
        Decimal::ONE
    }
    /// `TryModifyPowerAmountReceived(power, target, amount, giver)`: `Some(new amount)` if it
    /// changes it.
    fn try_modify_power_amount_received(
        &self,
        s: &CombatState,
        me: Self::Me,
        power: PowerQuery,
        amount: Decimal,
    ) -> Option<Decimal> {
        None
    }
    /// `ShouldAllowHitting(creature)`.
    fn should_allow_hitting(&self, s: &CombatState, me: Self::Me, creature: CreatureId) -> bool {
        true
    }
    /// `ShouldClearBlock(creature)`.
    fn should_clear_block(&self, s: &CombatState, me: Self::Me, creature: CreatureId) -> bool {
        true
    }
    /// `ShouldDraw(fromHandDraw)`.
    fn should_draw(&self, s: &CombatState, me: Self::Me, from_hand_draw: bool) -> bool {
        true
    }
    /// `ShouldPlay(card, autoPlayType)`: [`AutoPlayType::None`] for a manual play.
    fn should_play(
        &self,
        s: &CombatState,
        me: Self::Me,
        card: CardId,
        auto_play: AutoPlayType,
    ) -> bool {
        true
    }
    /// `ShouldPlayerResetEnergy`.
    fn should_player_reset_energy(&self, s: &CombatState, me: Self::Me) -> bool {
        true
    }
    /// `ShouldEtherealTrigger(card)`.
    fn should_ethereal_trigger(&self, s: &CombatState, me: Self::Me, card: CardId) -> bool {
        true
    }
    /// `ShouldFlush`.
    fn should_flush(&self, s: &CombatState, me: Self::Me) -> bool {
        true
    }
    /// `ShouldProcurePotion(potion, player)` (the only player procures; no listener reads the
    /// potion).
    fn should_procure_potion(&self, s: &CombatState, me: Self::Me) -> bool {
        true
    }
    /// `ShouldTakeExtraTurn(player)` (the only player takes the turn).
    fn should_take_extra_turn(&self, s: &CombatState, me: Self::Me) -> bool {
        false
    }
    /// `ShouldDie(creature)`.
    fn should_die(&self, s: &CombatState, me: Self::Me, creature: CreatureId) -> bool {
        true
    }
    /// `ShouldDieLate(creature)`.
    fn should_die_late(&self, s: &CombatState, me: Self::Me, creature: CreatureId) -> bool {
        true
    }
    /// `ShouldCreatureBeRemovedFromCombatAfterDeath(creature)`.
    fn should_creature_be_removed_after_death(
        &self,
        s: &CombatState,
        me: Self::Me,
        creature: CreatureId,
    ) -> bool {
        true
    }
    /// `ShouldPowerBeRemovedOnDeath(power)`.
    fn should_power_be_removed_on_death(
        &self,
        s: &CombatState,
        me: Self::Me,
        power: PowerRef,
    ) -> bool {
        true
    }
    /// `ShouldStopCombatFromEnding`.
    fn should_stop_combat_from_ending(&self, s: &CombatState, me: Self::Me) -> bool {
        false
    }
    /// `ModifyGoldGained(player, amount)`: returns the new running value.
    fn modify_gold_gained(&self, s: &CombatState, me: Self::Me, amount: Decimal) -> Decimal {
        amount
    }
    /// `ModifyOrbValue(orb, value)`: returns the new running value.
    fn modify_orb_value(&self, s: &CombatState, me: Self::Me, orb: Orb, value: Decimal) -> Decimal {
        value
    }
    /// `ModifyOrbPassiveTriggerCounts(orb, count)`: returns the new running count.
    fn modify_orb_passive_trigger_counts(
        &self,
        s: &CombatState,
        me: Self::Me,
        orb: Orb,
        count: i32,
    ) -> i32 {
        count
    }
}

/// A set of [`Hook`]s: the methods a model overrides.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HookSet([u64; Self::WORDS]);

impl_index!(Hook);

impl Hook {
    /// The word of a [`HookSet`] holding the hook, and its bit in it.
    #[inline]
    #[must_use]
    pub(crate) const fn word_mask(self) -> (usize, u64) {
        let bit = self.index();
        (bit / 64, 1 << (bit % 64))
    }
}

impl HookSet {
    const WORDS: usize = Hook::COUNT.div_ceil(64);
    pub const EMPTY: Self = Self([0; Self::WORDS]);
    pub const ALL: Self = Self([u64::MAX; Self::WORDS]);

    /// The set plus `hook`.
    #[must_use]
    pub const fn with(mut self, hook: Hook) -> Self {
        let bit = hook.index();
        self.0[bit / 64] |= 1 << (bit % 64);
        self
    }

    /// Whether `hook` is in the set.
    #[must_use]
    pub const fn contains(self, hook: Hook) -> bool {
        let bit = hook.index();
        self.0[bit / 64] & (1 << (bit % 64)) != 0
    }

    /// Whether `hook`, given as [`Hook::word_mask`], is in the set: one load and one test.
    #[inline]
    #[must_use]
    pub(crate) const fn has_word(&self, (word, mask): (usize, u64)) -> bool {
        self.0[word] & mask != 0
    }

    /// Whether the set has no hook.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        let mut i = 0;
        while i < Self::WORDS {
            if self.0[i] != 0 {
                return false;
            }
            i += 1;
        }
        true
    }

    /// Whether the sets share a hook.
    #[must_use]
    pub const fn intersects(self, other: Self) -> bool {
        let mut i = 0;
        while i < Self::WORDS {
            if self.0[i] & other.0[i] != 0 {
                return true;
            }
            i += 1;
        }
        false
    }

    /// The hooks in either set.
    #[must_use]
    pub fn union(mut self, other: Self) -> Self {
        for (word, other) in self.0.iter_mut().zip(other.0) {
            *word |= other;
        }
        self
    }

    /// The [`Hook::index`]es of the set's hooks, ascending.
    pub(crate) fn indices(self) -> impl Iterator<Item = usize> {
        self.0.into_iter().enumerate().flat_map(|(i, word)| {
            let mut rest = word;
            std::iter::from_fn(move || {
                (rest != 0).then(|| {
                    let bit = rest.trailing_zeros();
                    rest &= rest - 1;
                    i * 64 + usize::try_from(bit).expect("a bit index fits usize")
                })
            })
        })
    }

    /// Adds (`on`) or removes the hook with [`Hook::index`] `index`.
    pub(crate) const fn set_index(&mut self, index: usize, on: bool) {
        let bit = 1 << (index % 64);
        if on {
            self.0[index / 64] |= bit;
        } else {
            self.0[index / 64] &= !bit;
        }
    }
}

/// A listener as a pass visits it: its id, with its model's kind where the id fixes it for good
/// (a power instance's, a relic's, a potion's), so that the call needs no search. A monster's
/// model carries per-combat fields and a card's affliction or enchantment can change, so those
/// are read from the state at the call, as [`Listener`] would be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bound {
    Power(PowerKind, PowerRef),
    Monster(CreatureId),
    Relic(RelicKind, RelicId),
    Potion(PotionKind, PotionId),
    Card(CardId),
    Affliction(CardId),
    Enchantment(CardId),
}

impl Bound {
    /// The listener's id.
    #[inline]
    pub const fn listener(self) -> Listener {
        match self {
            Self::Power(_, me) => Listener::Power {
                owner: me.owner,
                power: me.id,
            },
            Self::Monster(id) => Listener::Monster(id),
            Self::Relic(_, id) => Listener::Relic(id),
            Self::Potion(_, id) => Listener::Potion(id),
            Self::Card(id) => Listener::Card(id),
            Self::Affliction(id) => Listener::Affliction(id),
            Self::Enchantment(id) => Listener::Enchantment(id),
        }
    }
}

/// Calls `$body` with `$m` bound to the listener's model and `$me` to its id: one dynamic call
/// per listener kind, however many models there are.
macro_rules! on_listener {
    ($s:expr, $bound:expr, $m:ident, $me:ident => $body:expr) => {
        match $bound {
            Bound::Power(kind, $me) => {
                let $m = kind.model();
                $body
            }
            Bound::Monster($me) => {
                let kind = $s
                    .creature($me)
                    .monster
                    .as_ref()
                    .expect("a monster listener has a monster")
                    .kind;
                let $m = kind.model();
                $body
            }
            Bound::Relic(kind, $me) => {
                let $m = kind.model();
                $body
            }
            Bound::Potion(kind, $me) => {
                let $m = kind.model();
                $body
            }
            Bound::Card($me) => {
                let $m = $s.card($me).kind.model();
                $body
            }
            Bound::Affliction($me) => {
                let $m = $s
                    .card($me)
                    .affliction
                    .expect("an afflicted card")
                    .kind
                    .model();
                $body
            }
            Bound::Enchantment($me) => {
                let $m = $s
                    .card($me)
                    .enchantment
                    .expect("an enchanted card")
                    .kind
                    .model();
                $body
            }
        }
    };
}

/// One pass of an async hook: `$method(eng, me, args...)` on every listener overriding it. A
/// hook no listener overrides costs one test here ([`Engine::lacks`]).
macro_rules! pass {
    ($eng:expr, $guard:expr, $method:ident($($arg:expr),*)) => {
        if $eng.lacks(Hook::$method) {
            Ok(())
        } else {
            $eng.dispatch($guard, Hook::$method, None, |eng, listener| {
                on_listener!(eng.s, listener, m, me => m.$method(eng, me $(, $arg)*))
            })
        }
    };
}

/// One pass of an async hook over the listeners in `$only` (an `AfterModifyingX` hook, or the
/// preventer of a `ShouldX` hook).
macro_rules! pass_to {
    ($eng:expr, $guard:expr, $only:expr, $method:ident($($arg:expr),*)) => {
        if $eng.lacks(Hook::$method) {
            Ok(())
        } else {
            $eng.dispatch($guard, Hook::$method, Some($only), |eng, listener| {
                on_listener!(eng.s, listener, m, me => m.$method(eng, me $(, $arg)*))
            })
        }
    };
}

/// A sync fold over the listeners overriding `$hook`, in order.
macro_rules! each {
    ($h:expr, $guard:expr, $hook:ident, |$m:ident, $me:ident| $body:expr) => {
        $h.visit(Hook::$hook, $guard, |bound| {
            on_listener!($h.s, bound, $m, $me => $body);
        })
    };
    ($h:expr, $guard:expr, $hook:ident, |$listener:ident, $m:ident, $me:ident| $body:expr) => {
        $h.visit(Hook::$hook, $guard, |bound| {
            let $listener = bound.listener();
            on_listener!($h.s, bound, $m, $me => $body);
        })
    };
}

/// The first listener (in order) whose `$method` returns false (`Option<Listener>`): the
/// preventer of a `Should*` predicate. The listeners after it are not called.
macro_rules! first_veto {
    ($h:expr, $guard:expr, $method:ident($($arg:expr),*)) => {{
        let mut veto = None;
        $h.visit(Hook::$method, $guard, |bound| {
            if veto.is_none()
                && !on_listener!($h.s, bound, m, me => m.$method($h.s, me $(, $arg)*))
            {
                veto = Some(bound.listener());
            }
        });
        veto
    }};
}

impl CombatState {
    /// The state as [`Hooked`], for the synchronous value hooks and the hook-aware queries.
    #[inline]
    pub(crate) const fn hooked(&self) -> Hooked<'_> {
        Hooked { s: self }
    }

    /// The `Contains` re-check made before a snapshot entry is yielded
    /// (`Combat/CombatState.cs:550-600`). A power is checked by its owner only (still in combat,
    /// a player's hooks active), not by whether it is still on the owner: one removed earlier in
    /// the same pass is still called (PX15: a Lightning evoke from `ConsumingShadowPower` kills
    /// the Slithering Strangler, whose death removes `ConstrictPower`, which still deals its
    /// damage in the same `AfterSideTurnEnd`).
    pub(crate) fn contains_listener(&self, listener: Listener) -> bool {
        match listener {
            Listener::Power { owner, .. } => {
                self.contains_creature(owner)
                    && (!self.creature(owner).is_player() || self.player.hooks_active)
            }
            Listener::Monster(id) => self.contains_creature(id),
            Listener::Relic(id) => {
                self.player.hooks_active && self.player.relics.iter().any(|r| r.id == id)
            }
            Listener::Potion(id) => {
                self.player.hooks_active && self.player.potions().any(|p| p.id == id)
            }
            Listener::Card(id) => self.player.hooks_active && self.pile_of(id).is_some(),
            Listener::Affliction(id) => {
                self.contains_listener(Listener::Card(id)) && self.card(id).affliction.is_some()
            }
            Listener::Enchantment(id) => {
                self.contains_listener(Listener::Card(id)) && self.card(id).enchantment.is_some()
            }
        }
    }

    /// The power instance `id` of `owner`, still on it or removed since: a listener keeps its
    /// instance after a removal mid-call (`PoisonPower.Trigger` on a Waterfall Giant whose death
    /// stripped its debuffs).
    pub(crate) fn power(&self, owner: CreatureId, id: PowerId) -> &Power {
        self.creature(owner)
            .power_instance(id)
            .expect("a power id names an instance of its owner")
    }

    /// `_internalData` of the power instance `id` (on its owner or removed since).
    #[must_use]
    #[inline]
    pub fn power_data(&self, id: PowerId) -> &PowerData {
        self.power_data.get(id)
    }

    /// `_internalData` of the power instance `id`, mutably.
    #[inline]
    pub(crate) fn power_data_mut(&mut self, id: PowerId) -> &mut PowerData {
        self.power_data.get_mut(id)
    }

    /// Sets `_internalData` of the power instance `id`.
    #[inline]
    pub(crate) fn set_power_data(&mut self, id: PowerId, data: PowerData) {
        self.power_data.set(id, data);
    }

    /// [`CombatState::power`], mutably.
    pub(crate) fn power_mut(&mut self, owner: CreatureId, id: PowerId) -> &mut Power {
        self.creature_mut(owner)
            .power_instance_mut(id)
            .expect("a power id names an instance of its owner")
    }

    /// `PlayerCombatState.MaxEnergy`: `(int)Hook.ModifyMaxEnergy(player.MaxEnergy)`
    /// (`PlayerCombatState.cs:101`, `Hook.cs:1778`, guarded).
    #[must_use]
    pub fn max_energy(&self) -> i32 {
        self.hooked().max_energy()
    }

    /// `CardEnergyCost.GetWithModifiers(All)` (`CardEnergyCost.cs:94-123`) of a combat card:
    /// the base cost as is if negative (unplayable) or X, else the local modifiers in order,
    /// then `max(0, (int)ModifyEnergyCostInCombat)`.
    #[must_use]
    pub fn energy_cost(&self, card: CardId) -> i32 {
        self.hooked().energy_cost(card)
    }

    /// `CardEnergyCost.GetAmountToSpend`: all the player's energy for an X cost, else
    /// `max(0, energy_cost)`.
    #[must_use]
    pub fn energy_to_spend(&self, card: CardId) -> i32 {
        self.hooked().energy_to_spend(card)
    }

    /// `CardModel.ResolveEnergyXValue` (`CardModel.cs:1117`): the captured X after
    /// `Hook.ModifyXValue`.
    ///
    /// # Panics
    /// If the card does not cost X.
    #[must_use]
    pub fn resolve_energy_x_value(&self, card: CardId) -> i32 {
        self.hooked().resolve_energy_x_value(card)
    }

    /// `CardModel.Keywords`: the card's local keywords plus the global ones from
    /// `Hook.ModifyKeywordsInCombat`.
    #[must_use]
    pub fn keywords(&self, card: CardId) -> Keywords {
        self.hooked().keywords(card)
    }

    /// `Keywords.Contains(keyword)`, global keywords included.
    #[must_use]
    pub fn has_keyword(&self, card: CardId, keyword: CardKeyword) -> bool {
        self.keywords(card).contains(keyword)
    }

    /// `ShouldRetainThisTurn`: the Retain keyword or a single-turn retain.
    #[must_use]
    pub fn should_retain_this_turn(&self, card: CardId) -> bool {
        self.hooked().should_retain_this_turn(card)
    }

    /// `IsSlyThisTurn`: the Sly keyword or a single-turn Sly.
    #[must_use]
    pub fn is_sly_this_turn(&self, card: CardId) -> bool {
        self.hooked().is_sly_this_turn(card)
    }

    /// `CardModel.GetStarCostWithModifiers` (`CardModel.cs:1556-1568`): all the player's stars
    /// for an X star cost; else `CurrentStarCost`, through `Hook.ModifyStarCost` for a card in a
    /// combat pile. -1 (or below) means no star cost.
    #[must_use]
    pub fn star_cost(&self, card: CardId) -> i32 {
        self.hooked().star_cost(card)
    }

    /// `CardModel.CostsEnergyOrStars` (`CardModel.cs:1579-1594`): whether the card has an energy
    /// or star cost above 0, with the global modifiers or (`false`) only the local ones.
    #[must_use]
    pub fn costs_energy_or_stars(&self, card: CardId, include_global_modifiers: bool) -> bool {
        self.hooked()
            .costs_energy_or_stars(card, include_global_modifiers)
    }

    /// `CardModel.ResolveStarXValue` (`CardModel.cs:1139`): `LastStarsSpent` after
    /// `Hook.ModifyXValue`.
    ///
    /// # Panics
    /// If the card does not cost X stars.
    #[must_use]
    pub fn resolve_star_x_value(&self, card: CardId) -> i32 {
        self.hooked().resolve_star_x_value(card)
    }

    /// The `TargetType` of a combat card ([`CardModel::target_in`]).
    ///
    /// [`CardModel::target_in`]: crate::content::cards::CardModel
    #[must_use]
    pub fn target_type(&self, card: CardId) -> crate::content::cards::TargetType {
        crate::content::cards::target_type(self, card)
    }

    /// `Hook.ShouldStopCombatFromEnding` ([`Hooked::should_stop_combat_from_ending`]).
    pub(crate) fn should_stop_combat_from_ending(&self) -> bool {
        self.hooked().should_stop_combat_from_ending()
    }
}

/// The state, for the synchronous value hooks and the hook-aware queries
/// ([`Combat::hooked`](super::Combat::hooked)). The listeners and the union of their hooks are
/// read from the state ([`super::listeners`]).
#[derive(Clone, Copy, Debug)]
pub struct Hooked<'a> {
    /// The state the listeners belong to.
    pub s: &'a CombatState,
}

impl Hooked<'_> {
    /// Whether some listener overrides `hook`.
    #[inline]
    #[must_use]
    pub fn has(self, hook: Hook) -> bool {
        !self.s.lacks(hook)
    }

    /// Calls `visit` on the listeners overriding `hook`, in order, honouring `guard`.
    #[inline]
    fn visit(self, hook: Hook, guard: Guard, visit: impl FnMut(Bound)) {
        if self.has(hook) && !self.is_guard_closed(guard) {
            self.s.visit_listeners(hook, visit);
        }
    }

    /// Whether some listener overrides one of `hooks`.
    fn any_hook(self, hooks: &[Hook]) -> bool {
        hooks.iter().any(|&hook| self.has(hook))
    }

    /// Whether a guarded dispatch starting now sees no listeners (`Hook.cs:55`).
    pub(crate) fn is_guard_closed(self, guard: Guard) -> bool {
        guard == Guard::Combat && !self.s.starting && self.is_over_or_ending()
    }

    /// [`CombatState::is_ending`] on these listeners.
    pub(crate) fn is_ending(self) -> bool {
        self.s
            .is_ending_unless(|| self.should_stop_combat_from_ending())
    }

    /// [`CombatState::is_over_or_ending`] on these listeners.
    pub(crate) fn is_over_or_ending(self) -> bool {
        !self.s.in_progress || self.is_ending()
    }

    /// `HittableEnemies` ([`CombatState::hittable_enemies`]) on these listeners.
    #[must_use]
    pub fn hittable_enemies(self) -> Creatures {
        self.s
            .enemies
            .iter()
            .copied()
            .filter(|&e| self.s.creature(e).is_alive() && self.should_allow_hitting(e))
            .collect()
    }

    /// `CardModel.CostsEnergyOrStars` ([`CombatState::costs_energy_or_stars`]) on these
    /// listeners.
    #[must_use]
    pub fn costs_energy_or_stars(self, card: CardId, include_global_modifiers: bool) -> bool {
        let s = self.s;
        let c = s.card(card);
        if include_global_modifiers {
            (!c.energy_cost.costs_x && self.energy_cost(card) > 0)
                || (!c.star_cost.costs_x && self.star_cost(card) > 0)
        } else {
            s.local_energy_cost(card).unwrap_or_else(|base| base) > 0
                || s.current_star_cost(card) > 0
        }
    }

    /// `CardModel.ResolveStarXValue` ([`CombatState::resolve_star_x_value`]) on these
    /// listeners.
    ///
    /// # Panics
    /// If the card does not cost X stars.
    #[must_use]
    pub fn resolve_star_x_value(self, card: CardId) -> i32 {
        let cost = &self.s.card(card).star_cost;
        assert!(cost.costs_x, "This card does not have an X-cost.");
        self.modify_x_value(card, cost.last_spent)
    }

    /// `Hook.ModifyDamage(All)` (`Hook.cs:1495`, `ModifyDamageInternal` `:2519`): additive
    /// pass, multiplicative pass (each factor applied in listener order), cap pass, then
    /// `max(0, x)`. Returns the damage and the listeners that changed it. The list is only
    /// read by `AfterModifyingDamageAmount`, so it stays empty (no allocation) while no listener
    /// overrides that. The multi-target preview branch is UI only.
    #[inline]
    pub(crate) fn modify_damage(
        self,
        query: DamageQuery,
        amount: Decimal,
    ) -> (Decimal, Vec<Listener>) {
        let enchantment = query.card.and_then(|c| self.s.card(c).enchantment);
        // Nothing to fold: the passes would leave the amount as is.
        if enchantment.is_none()
            && !self.any_hook(&[
                Hook::modify_damage_additive,
                Hook::modify_damage_multiplicative,
                Hook::modify_damage_cap,
            ])
        {
            return (amount.max(Decimal::ZERO), Vec::new());
        }
        self.fold_damage(query, amount, enchantment)
    }

    /// [`Hooked::modify_damage`] with a listener or an enchantment.
    #[inline(never)]
    fn fold_damage(
        self,
        query: DamageQuery,
        amount: Decimal,
        enchantment: Option<crate::content::enchantments::Enchantment>,
    ) -> (Decimal, Vec<Listener>) {
        let collect = self.has(Hook::after_modifying_damage_amount);
        let mut modifiers = Vec::new();
        let mut damage = amount;
        // The card source's enchantment first; it does not count as a modifier.
        if let Some(enchantment) = enchantment {
            let m = enchantment.kind.model();
            damage += m.enchant_damage_additive(enchantment, damage, query.props);
            damage *= m.enchant_damage_multiplicative(enchantment, damage, query.props);
        }
        each!(
            self,
            Guard::None,
            modify_damage_additive,
            |listener, m, me| {
                let add = m.modify_damage_additive(self.s, me, query, damage);
                damage += add;
                if collect && add != Decimal::ZERO {
                    modifiers.push(listener);
                }
            }
        );
        each!(
            self,
            Guard::None,
            modify_damage_multiplicative,
            |listener, m, me| {
                let factor = m.modify_damage_multiplicative(self.s, me, query, damage);
                damage *= factor;
                if collect && factor != Decimal::ONE {
                    modifiers.push(listener);
                }
            }
        );
        let mut cap = Decimal::MAX;
        each!(self, Guard::None, modify_damage_cap, |listener, m, me| {
            let listener_cap = m.modify_damage_cap(self.s, me, query);
            if listener_cap < cap {
                cap = listener_cap;
                if damage > listener_cap {
                    damage = listener_cap;
                    if collect {
                        modifiers.push(listener);
                    }
                }
            }
        });
        (damage.max(Decimal::ZERO), modifiers)
    }

    /// `Hook.ModifyHpLost` for one phase (`Hook.cs:1725`): the normal pass then the `Late` pass,
    /// threading the value; a listener counts as a modifier when it changed `Truncate(x)`.
    #[inline]
    pub(crate) fn modify_hp_lost(
        self,
        query: HpLossQuery,
        amount: Decimal,
        phase: HpLossPhase,
    ) -> (Decimal, Vec<Listener>) {
        let hooks = match phase {
            HpLossPhase::BeforeOsty => [
                Hook::modify_hp_lost_before_osty,
                Hook::modify_hp_lost_before_osty_late,
            ],
            HpLossPhase::AfterOsty => [
                Hook::modify_hp_lost_after_osty,
                Hook::modify_hp_lost_after_osty_late,
            ],
        };
        if self.any_hook(&hooks) {
            self.fold_hp_lost(query, amount, phase)
        } else {
            (amount, Vec::new())
        }
    }

    /// [`Hooked::modify_hp_lost`] with a listener.
    #[inline(never)]
    fn fold_hp_lost(
        self,
        query: HpLossQuery,
        amount: Decimal,
        phase: HpLossPhase,
    ) -> (Decimal, Vec<Listener>) {
        // Only `AfterModifyingHpLost<phase>` reads the list ([`Hooked::modify_damage`]).
        let collect = self.has(match phase {
            HpLossPhase::BeforeOsty => Hook::after_modifying_hp_lost_before_osty,
            HpLossPhase::AfterOsty => Hook::after_modifying_hp_lost_after_osty,
        });
        let mut modifiers = Vec::new();
        let mut value = amount;
        let mut step = |listener: Listener, new: Decimal, value: &mut Decimal| {
            if collect && value.truncate() != new.truncate() {
                modifiers.push(listener);
            }
            *value = new;
        };
        let s = self.s;
        match phase {
            HpLossPhase::BeforeOsty => {
                each!(
                    self,
                    Guard::None,
                    modify_hp_lost_before_osty,
                    |listener, m, me| {
                        let new = m.modify_hp_lost_before_osty(s, me, query, value);
                        step(listener, new, &mut value);
                    }
                );
                each!(
                    self,
                    Guard::None,
                    modify_hp_lost_before_osty_late,
                    |listener, m, me| {
                        let new = m.modify_hp_lost_before_osty_late(s, me, query, value);
                        step(listener, new, &mut value);
                    }
                );
            }
            HpLossPhase::AfterOsty => {
                each!(
                    self,
                    Guard::None,
                    modify_hp_lost_after_osty,
                    |listener, m, me| {
                        let new = m.modify_hp_lost_after_osty(s, me, query, value);
                        step(listener, new, &mut value);
                    }
                );
                each!(
                    self,
                    Guard::None,
                    modify_hp_lost_after_osty_late,
                    |listener, m, me| {
                        let new = m.modify_hp_lost_after_osty_late(s, me, query, value);
                        step(listener, new, &mut value);
                    }
                );
            }
        }
        (value, modifiers)
    }

    /// `Hook.ModifyUnblockedDamageTarget` (`Hook.cs:2056`, unguarded): threads the target
    /// through every listener.
    pub(crate) fn modify_unblocked_damage_target(
        self,
        target: CreatureId,
        amount: Decimal,
        props: ValueProps,
        dealer: Option<CreatureId>,
    ) -> CreatureId {
        if !self.any_hook(&[Hook::modify_unblocked_damage_target]) {
            return target;
        }
        let mut target = target;
        each!(
            self,
            Guard::None,
            modify_unblocked_damage_target,
            |m, me| {
                target =
                    m.modify_unblocked_damage_target(self.s, me, target, amount, props, dealer);
            }
        );
        target
    }

    /// `Hook.ModifyBlock` (`Hook.cs:1322`, guarded): additive pass, multiplicative pass, then
    /// `max(0, x)`. Returns the block and the listeners that changed it. No in-scope card has an
    /// enchantment.
    #[inline]
    pub(crate) fn modify_block(
        self,
        query: BlockQuery,
        amount: Decimal,
    ) -> (Decimal, Vec<Listener>) {
        let enchantment = query.card.and_then(|c| self.s.card(c).enchantment);
        // Nothing to fold (the guard would not matter either).
        let listened = self.any_hook(&[
            Hook::modify_block_additive,
            Hook::modify_block_multiplicative,
        ]);
        if enchantment.is_none() && !listened {
            return (amount.max(Decimal::ZERO), Vec::new());
        }
        self.fold_block(query, amount, enchantment)
    }

    /// [`Hooked::modify_block`] with a listener or an enchantment.
    #[inline(never)]
    fn fold_block(
        self,
        query: BlockQuery,
        amount: Decimal,
        enchantment: Option<crate::content::enchantments::Enchantment>,
    ) -> (Decimal, Vec<Listener>) {
        // Only `AfterModifyingBlockAmount` reads the list ([`Hooked::modify_damage`]).
        let collect = self.has(Hook::after_modifying_block_amount);
        let mut modifiers = Vec::new();
        let mut block = amount;
        if self.is_guard_closed(Guard::Combat) {
            return (block.max(Decimal::ZERO), modifiers);
        }
        // The card source's enchantment first; it does not count as a modifier.
        if let Some(enchantment) = enchantment {
            let m = enchantment.kind.model();
            block += m.enchant_block_additive(enchantment, block);
            block *= m.enchant_block_multiplicative(enchantment, block);
        }
        each!(
            self,
            Guard::None,
            modify_block_additive,
            |listener, m, me| {
                let add = m.modify_block_additive(self.s, me, query, block);
                block += add;
                if collect && add != Decimal::ZERO {
                    modifiers.push(listener);
                }
            }
        );
        each!(
            self,
            Guard::None,
            modify_block_multiplicative,
            |listener, m, me| {
                let factor = m.modify_block_multiplicative(self.s, me, query, block);
                block *= factor;
                if collect && factor != Decimal::ONE {
                    modifiers.push(listener);
                }
            }
        );
        (block.max(Decimal::ZERO), modifiers)
    }

    /// `Hook.ModifyHandDraw` (`Hook.cs:1692`, guarded): normal pass then `Late` pass; a
    /// listener counts as a modifier when it changed `(int)x`.
    pub(crate) fn modify_hand_draw(self, count: Decimal) -> (Decimal, Vec<Listener>) {
        let mut modifiers = Vec::new();
        let mut value = count;
        if self.is_guard_closed(Guard::Combat) {
            return (value, modifiers);
        }
        each!(self, Guard::None, modify_hand_draw, |listener, m, me| {
            let new = m.modify_hand_draw(self.s, me, value);
            if new.to_int_truncating() != value.to_int_truncating() {
                modifiers.push(listener);
            }
            value = new;
        });
        each!(
            self,
            Guard::None,
            modify_hand_draw_late,
            |listener, m, me| {
                let new = m.modify_hand_draw_late(self.s, me, value);
                if new.to_int_truncating() != value.to_int_truncating() {
                    modifiers.push(listener);
                }
                value = new;
            }
        );
        (value, modifiers)
    }

    /// `Hook.ModifyShuffleOrder` (`Hook.cs:2012`, guarded): each listener reorders `cards` in
    /// turn.
    pub(crate) fn modify_shuffle_order(self, cards: &mut Vec<CardId>, is_initial: bool) {
        each!(self, Guard::Combat, modify_shuffle_order, |m, me| {
            m.modify_shuffle_order(self.s, me, cards, is_initial);
        });
    }

    /// `PlayerCombatState.MaxEnergy`: `(int)Hook.ModifyMaxEnergy(player.MaxEnergy)`
    /// (`PlayerCombatState.cs:101`, `Hook.cs:1778`, guarded).
    #[must_use]
    pub fn max_energy(self) -> i32 {
        let mut amount = Decimal::from(self.s.player.max_energy);
        each!(self, Guard::Combat, modify_max_energy, |m, me| {
            amount = m.modify_max_energy(self.s, me, amount);
        });
        amount.to_int_truncating()
    }

    /// `Hook.ModifyCardPlayResultLocation` (`Hook.cs:1403`, guarded). Returns the location and
    /// the listeners that changed it.
    pub(crate) fn modify_card_play_result_location(
        self,
        card: CardId,
        is_auto_play: bool,
        resources: Resources,
        location: ResultLocation,
    ) -> (ResultLocation, Vec<Listener>) {
        let mut modifiers = Vec::new();
        let mut value = location;
        each!(
            self,
            Guard::Combat,
            modify_card_play_result_location,
            |listener, m, me| {
                let new = m.modify_card_play_result_location(
                    self.s,
                    me,
                    card,
                    is_auto_play,
                    resources,
                    value,
                );
                if new != value {
                    modifiers.push(listener);
                }
                value = new;
            }
        );
        (value, modifiers)
    }

    /// `Hook.ModifyCardPlayCount` (`Hook.cs:1384`, guarded). Returns the count and the
    /// listeners that changed it.
    #[inline]
    pub(crate) fn modify_card_play_count(
        self,
        card: CardId,
        target: Option<CreatureId>,
        count: u32,
    ) -> (u32, Vec<Listener>) {
        if self.has(Hook::modify_card_play_count) {
            self.fold_card_play_count(card, target, count)
        } else {
            (count, Vec::new())
        }
    }

    /// [`Hooked::modify_card_play_count`] with a listener.
    #[inline(never)]
    fn fold_card_play_count(
        self,
        card: CardId,
        target: Option<CreatureId>,
        count: u32,
    ) -> (u32, Vec<Listener>) {
        let mut modifiers = Vec::new();
        let mut value = count;
        each!(
            self,
            Guard::Combat,
            modify_card_play_count,
            |listener, m, me| {
                let new = m.modify_card_play_count(self.s, me, card, target, value);
                if new != value {
                    modifiers.push(listener);
                }
                value = new;
            }
        );
        (value, modifiers)
    }

    /// `Hook.ModifyEnergyCostInCombat` (`Hook.cs:1582`, guarded): a negative cost is returned
    /// unchanged; otherwise the `TryModify` pass, then the `Late` pass.
    pub(crate) fn modify_energy_cost_in_combat(self, card: CardId, cost: Decimal) -> Decimal {
        // Without a listener both passes are empty, so the guard need not be asked.
        let listened = self.has(Hook::try_modify_energy_cost_in_combat)
            || self.has(Hook::try_modify_energy_cost_in_combat_late);
        if cost < Decimal::ZERO || !listened || self.is_guard_closed(Guard::Combat) {
            return cost;
        }
        let mut value = cost;
        each!(
            self,
            Guard::None,
            try_modify_energy_cost_in_combat,
            |m, me| {
                if let Some(new) = m.try_modify_energy_cost_in_combat(self.s, me, card, value) {
                    value = new;
                }
            }
        );
        each!(
            self,
            Guard::None,
            try_modify_energy_cost_in_combat_late,
            |m, me| {
                if let Some(new) = m.try_modify_energy_cost_in_combat_late(self.s, me, card, value)
                {
                    value = new;
                }
            }
        );
        value
    }

    /// `CardEnergyCost.GetWithModifiers(All)` (`CardEnergyCost.cs:94-123`) of a combat card:
    /// the base cost as is if negative (unplayable) or X, else the local modifiers in order,
    /// then `max(0, (int)ModifyEnergyCostInCombat)`.
    #[must_use]
    pub fn energy_cost(self, card: CardId) -> i32 {
        match self.s.local_energy_cost(card) {
            Err(base) => base,
            // No listener: `ModifyEnergyCostInCombat` returns the integral cost unchanged.
            Ok(cost)
                if !self.has(Hook::try_modify_energy_cost_in_combat)
                    && !self.has(Hook::try_modify_energy_cost_in_combat_late) =>
            {
                cost.max(0)
            }
            Ok(cost) => self
                .modify_energy_cost_in_combat(card, Decimal::from(cost))
                .to_int_truncating()
                .max(0),
        }
    }

    /// `CardEnergyCost.GetAmountToSpend`: all the player's energy for an X cost, else
    /// `max(0, energy_cost)`.
    #[must_use]
    pub fn energy_to_spend(self, card: CardId) -> i32 {
        if self.s.card(card).energy_cost.costs_x {
            self.s.player.energy
        } else {
            self.energy_cost(card).max(0)
        }
    }

    /// `CardModel.ResolveEnergyXValue` (`CardModel.cs:1117`): the captured X after
    /// `Hook.ModifyXValue`.
    ///
    /// # Panics
    /// If the card does not cost X.
    #[must_use]
    pub fn resolve_energy_x_value(self, card: CardId) -> i32 {
        let cost = &self.s.card(card).energy_cost;
        assert!(cost.costs_x, "This card does not have an X-cost.");
        self.modify_x_value(card, cost.captured_x)
    }

    /// `Hook.ModifyStarCost` (`Hook.cs:2023`, guarded): a negative cost is returned unchanged;
    /// otherwise each listener's `TryModifyStarCost` in turn.
    pub(crate) fn modify_star_cost(self, card: CardId, cost: Decimal) -> Decimal {
        if cost < Decimal::ZERO {
            return cost;
        }
        let mut value = cost;
        each!(self, Guard::Combat, try_modify_star_cost, |m, me| {
            if let Some(new) = m.try_modify_star_cost(self.s, me, card, value) {
                value = new;
            }
        });
        value
    }

    /// `CardModel.GetStarCostWithModifiers` ([`CombatState::star_cost`]). Every engine pile is
    /// a combat pile.
    #[must_use]
    pub fn star_cost(self, card: CardId) -> i32 {
        let c = self.s.card(card);
        if c.star_cost.costs_x {
            return self.s.player.stars;
        }
        // No star cost (most cards): `ModifyStarCost` returns a negative cost unchanged.
        let temporary = self.s.cost_lists.star(card);
        if c.star_cost.base < 0 && temporary.is_empty() {
            return c.star_cost.base;
        }
        let current = c.star_cost.current(temporary);
        // No listener: `ModifyStarCost` returns the integral cost unchanged.
        if c.pile.is_some() && self.has(Hook::try_modify_star_cost) {
            self.modify_star_cost(card, Decimal::from(current))
                .to_int_truncating()
        } else {
            current
        }
    }

    /// `Hook.ShouldGainStars` (`Hook.cs:2339`, guarded): false on the first veto.
    pub(crate) fn should_gain_stars(self, amount: Decimal) -> bool {
        first_veto!(self, Guard::Combat, should_gain_stars(amount)).is_none()
    }

    /// `Hook.ModifyXValue` (`Hook.cs:2079`, guarded), for `CardModel.ResolveEnergyXValue`.
    #[must_use]
    pub fn modify_x_value(self, card: CardId, value: i32) -> i32 {
        let mut value = value;
        each!(self, Guard::Combat, modify_x_value, |m, me| {
            value = m.modify_x_value(self.s, me, card, value);
        });
        value
    }

    /// `Hook.ModifyPowerAmountGiven` (`Hook.cs:1896`, guarded): additive then multiplicative
    /// pass. Returns the amount and the listeners that changed it.
    #[inline]
    pub(crate) fn modify_power_amount_given(
        self,
        power: PowerQuery,
        amount: Decimal,
    ) -> (Decimal, Vec<Listener>) {
        // Without a listener both passes are empty, so the guard need not be asked.
        if self.any_hook(&[
            Hook::modify_power_amount_given_additive,
            Hook::modify_power_amount_given_multiplicative,
        ]) {
            self.fold_power_amount_given(power, amount)
        } else {
            (amount, Vec::new())
        }
    }

    /// [`Hooked::modify_power_amount_given`] with a listener.
    #[inline(never)]
    fn fold_power_amount_given(
        self,
        power: PowerQuery,
        amount: Decimal,
    ) -> (Decimal, Vec<Listener>) {
        let mut modifiers = Vec::new();
        let mut value = amount;
        if self.is_guard_closed(Guard::Combat) {
            return (value, modifiers);
        }
        each!(
            self,
            Guard::None,
            modify_power_amount_given_additive,
            |listener, m, me| {
                let add = m.modify_power_amount_given_additive(self.s, me, power, value);
                value += add;
                if add != Decimal::ZERO {
                    modifiers.push(listener);
                }
            }
        );
        each!(
            self,
            Guard::None,
            modify_power_amount_given_multiplicative,
            |listener, m, me| {
                let factor = m.modify_power_amount_given_multiplicative(self.s, me, power, value);
                value *= factor;
                if factor != Decimal::ONE {
                    modifiers.push(listener);
                }
            }
        );
        (value, modifiers)
    }

    /// `Hook.ModifyPowerAmountReceived` (`Hook.cs:1925`, guarded). Returns the amount and the
    /// listeners that changed it.
    #[inline]
    pub(crate) fn modify_power_amount_received(
        self,
        power: PowerQuery,
        amount: Decimal,
    ) -> (Decimal, Vec<Listener>) {
        if self.has(Hook::try_modify_power_amount_received) {
            self.fold_power_amount_received(power, amount)
        } else {
            (amount, Vec::new())
        }
    }

    /// [`Hooked::modify_power_amount_received`] with a listener.
    #[inline(never)]
    fn fold_power_amount_received(
        self,
        power: PowerQuery,
        amount: Decimal,
    ) -> (Decimal, Vec<Listener>) {
        let mut modifiers = Vec::new();
        let mut value = amount;
        each!(
            self,
            Guard::Combat,
            try_modify_power_amount_received,
            |listener, m, me| {
                if let Some(new) = m.try_modify_power_amount_received(self.s, me, power, value) {
                    value = new;
                    modifiers.push(listener);
                }
            }
        );
        (value, modifiers)
    }

    /// `Hook.ShouldAllowHitting` (`Hook.cs:2139`, guarded): false on the first veto. Gates
    /// `CanReceivePowers`.
    pub(crate) fn should_allow_hitting(self, creature: CreatureId) -> bool {
        first_veto!(self, Guard::Combat, should_allow_hitting(creature)).is_none()
    }

    /// `Hook.ShouldClearBlock` (`Hook.cs:2201`, guarded): the preventer, if any.
    pub(crate) fn block_clear_preventer(self, creature: CreatureId) -> Option<Listener> {
        first_veto!(self, Guard::Combat, should_clear_block(creature))
    }

    /// `Hook.ShouldDraw` (`Hook.cs:2277`, guarded): the preventer, if any.
    pub(crate) fn draw_preventer(self, from_hand_draw: bool) -> Option<Listener> {
        first_veto!(self, Guard::Combat, should_draw(from_hand_draw))
    }

    /// `Hook.ShouldPlay(card, autoPlayType)` (`Hook.cs:2369`, guarded).
    #[inline]
    pub(crate) fn should_play(self, card: CardId, auto_play: AutoPlayType) -> bool {
        !self.has(Hook::should_play) || self.no_play_veto(card, auto_play)
    }

    /// [`Hooked::should_play`] with a listener.
    #[inline(never)]
    fn no_play_veto(self, card: CardId, auto_play: AutoPlayType) -> bool {
        first_veto!(self, Guard::Combat, should_play(card, auto_play)).is_none()
    }

    /// The first listener whose `ShouldPlay` vetoes `card` (`Hook.ShouldPlay`'s `preventer`).
    pub(crate) fn play_preventer(self, card: CardId, auto_play: AutoPlayType) -> Option<Listener> {
        first_veto!(self, Guard::Combat, should_play(card, auto_play))
    }

    /// `Hook.ShouldPlayerResetEnergy` (`Hook.cs:2386`, guarded).
    pub(crate) fn should_player_reset_energy(self) -> bool {
        first_veto!(self, Guard::Combat, should_player_reset_energy()).is_none()
    }

    /// `CardModel.Keywords` (`GetKeywordsWithSources(All)`, `CardModel.cs:1153-1181`): the
    /// local keywords, then `Hook.ModifyKeywordsInCombat` (`Hook.cs:1603`, unguarded) lets every
    /// listener add or remove global ones.
    #[inline]
    #[must_use]
    pub fn keywords(self, card: CardId) -> Keywords {
        let keywords = self.s.card(card).local_keywords();
        if self.has(Hook::try_modify_keywords_in_combat) {
            self.fold_keywords(card, keywords)
        } else {
            keywords
        }
    }

    /// [`Hooked::keywords`] with a listener.
    #[inline(never)]
    fn fold_keywords(self, card: CardId, mut keywords: Keywords) -> Keywords {
        each!(self, Guard::None, try_modify_keywords_in_combat, |m, me| {
            m.try_modify_keywords_in_combat(self.s, me, card, &mut keywords);
        });
        keywords
    }

    /// `Keywords.Contains(keyword)`.
    #[inline]
    #[must_use]
    pub fn has_keyword(self, card: CardId, keyword: CardKeyword) -> bool {
        self.keywords(card).contains(keyword)
    }

    /// `ShouldRetainThisTurn`: the Retain keyword or a single-turn retain.
    #[must_use]
    pub fn should_retain_this_turn(self, card: CardId) -> bool {
        self.has_keyword(card, CardKeyword::Retain) || self.s.card(card).single_turn_retain
    }

    /// `IsSlyThisTurn`: the Sly keyword or a single-turn Sly.
    #[must_use]
    pub fn is_sly_this_turn(self, card: CardId) -> bool {
        self.has_keyword(card, CardKeyword::Sly) || self.s.card(card).single_turn_sly
    }

    /// `Hook.ShouldEtherealTrigger` (`Hook.cs:2294`, guarded).
    pub(crate) fn should_ethereal_trigger(self, card: CardId) -> bool {
        first_veto!(self, Guard::Combat, should_ethereal_trigger(card)).is_none()
    }

    /// `Hook.ShouldTakeExtraTurn` (`Hook.cs:2465`, guarded): whether any listener says yes.
    pub(crate) fn should_take_extra_turn(self) -> bool {
        let mut yes = false;
        self.visit(Hook::should_take_extra_turn, Guard::Combat, |bound| {
            yes = yes || on_listener!(self.s, bound, m, me => m.should_take_extra_turn(self.s, me));
        });
        yes
    }

    /// `Hook.ShouldFlush` (`Hook.cs:2309`, guarded).
    pub(crate) fn should_flush(self) -> bool {
        first_veto!(self, Guard::Combat, should_flush()).is_none()
    }

    /// `Hook.ShouldProcurePotion` (`Hook.cs:2416-2426`, unguarded): no listener vetoes.
    pub(crate) fn should_procure_potion(self) -> bool {
        first_veto!(self, Guard::None, should_procure_potion()).is_none()
    }

    /// `Hook.ShouldDie` (`Hook.cs:2237`, unguarded): pass `ShouldDie`, then `ShouldDieLate`;
    /// the preventer, if any.
    pub(crate) fn death_preventer(self, creature: CreatureId) -> Option<Listener> {
        first_veto!(self, Guard::None, should_die(creature))
            .or_else(|| first_veto!(self, Guard::None, should_die_late(creature)))
    }

    /// `Hook.ShouldCreatureBeRemovedFromCombatAfterDeath` (`Hook.cs:2222`, unguarded): false on
    /// the first veto.
    pub(crate) fn should_creature_be_removed_after_death(self, creature: CreatureId) -> bool {
        first_veto!(
            self,
            Guard::None,
            should_creature_be_removed_after_death(creature)
        )
        .is_none()
    }

    /// `Hook.ShouldPowerBeRemovedOnDeath` (`Hook.cs:2503`, unguarded).
    pub(crate) fn should_power_be_removed_on_death(self, power: PowerRef) -> bool {
        first_veto!(self, Guard::None, should_power_be_removed_on_death(power)).is_none()
    }

    /// `Hook.ShouldStopCombatFromEnding` (`Hook.cs:2450`, unguarded): true on the first vote.
    pub(crate) fn should_stop_combat_from_ending(self) -> bool {
        let mut yes = false;
        self.visit(Hook::should_stop_combat_from_ending, Guard::None, |bound| {
            yes = yes
                || on_listener!(self.s, bound, m, me => m.should_stop_combat_from_ending(self.s, me));
        });
        yes
    }

    /// `Hook.ModifyOrbValue` (`Hook.cs:1882-1890`, guarded): a fold over every listener.
    pub(crate) fn modify_orb_value(self, orb: Orb, value: Decimal) -> Decimal {
        let mut value = value;
        each!(self, Guard::Combat, modify_orb_value, |m, me| {
            value = m.modify_orb_value(self.s, me, orb, value);
        });
        value
    }

    /// `Hook.ModifyOrbPassiveTriggerCount` (`Hook.cs:1863-1876`, guarded). Returns the count and
    /// the listeners that changed it.
    pub(crate) fn modify_orb_passive_trigger_count(
        self,
        orb: Orb,
        count: i32,
    ) -> (i32, Vec<Listener>) {
        let mut modifiers = Vec::new();
        let mut count = count;
        each!(
            self,
            Guard::Combat,
            modify_orb_passive_trigger_counts,
            |listener, m, me| {
                let new = m.modify_orb_passive_trigger_counts(self.s, me, orb, count);
                if new != count {
                    modifiers.push(listener);
                }
                count = new;
            }
        );
        (count, modifiers)
    }

    /// `Hook.ModifyGoldGained` (`Hook.cs:1634`, run-level with the combat): fold over the
    /// listeners; a listener counts as a modifier when it changed `(int)` gold.
    pub(crate) fn modify_gold_gained(self, amount: Decimal) -> (Decimal, Vec<Listener>) {
        let mut modifiers = Vec::new();
        let mut gold = amount;
        each!(self, Guard::None, modify_gold_gained, |listener, m, me| {
            let before = gold;
            gold = m.modify_gold_gained(self.s, me, gold);
            if before.to_int_truncating() != gold.to_int_truncating() {
                modifiers.push(listener);
            }
        });
        (gold, modifiers)
    }
}

impl Engine<'_> {
    /// One pass of an async hook over a snapshot of the listeners overriding `hook`,
    /// restricted to `only` if given. Each entry is re-checked with
    /// [`CombatState::contains_listener`] just before its call.
    #[inline(never)]
    pub(crate) fn dispatch(
        &mut self,
        guard: Guard,
        hook: Hook,
        only: Option<&[Listener]>,
        mut call: impl FnMut(&mut Self, Bound) -> Flow,
    ) -> Flow {
        let hooked = self.hooked();
        if !hooked.has(hook) || hooked.is_guard_closed(guard) {
            return Ok(());
        }
        let mut snapshot = Listeners::new();
        self.s.visit_listeners(hook, |bound| {
            if only.is_none_or(|o| o.contains(&bound.listener())) {
                snapshot.push(bound);
            }
        });
        // By reference: moving the list into an iterator copies it right after it was built.
        for &bound in &snapshot {
            if self.s.contains_listener(bound.listener()) {
                call(self, bound)?;
            }
        }
        Ok(())
    }

    /// `Hook.AfterModifyingGoldGained` (`Hook.cs:736`): the modifiers only.
    #[inline]
    pub(crate) fn after_modifying_gold_gained(
        &mut self,
        modifiers: &[Listener],
        amount: Decimal,
    ) -> Flow {
        pass_to!(
            self,
            Guard::None,
            modifiers,
            after_modifying_gold_gained(amount)
        )
    }

    /// `Hook.AfterGoldGained` (`Hook.cs:481`): run-level listeners without the combat (the
    /// relics, then the potions by slot).
    pub(crate) fn after_gold_gained(&mut self) -> Flow {
        let mut listeners: Vec<Bound> = self
            .s
            .player
            .relics
            .iter()
            .filter(|r| !r.melted)
            .map(|r| Bound::Relic(r.kind, r.id))
            .collect();
        listeners.extend(self.s.player.potions().map(|p| Bound::Potion(p.kind, p.id)));
        for bound in listeners {
            let present = match bound {
                Bound::Relic(_, id) => self.s.player.relics.iter().any(|r| r.id == id),
                Bound::Potion(_, id) => self.s.player.potions().any(|p| p.id == id),
                _ => unreachable!("only relics and potions"),
            };
            if present {
                on_listener!(self.s, bound, m, me => m.after_gold_gained(self, me))?;
            }
        }
        Ok(())
    }

    /// `Hook.AfterRoomEntered` (`Hook.cs:1128`): run-level listeners without the combat, i.e.
    /// the relics, then the potions by slot (the engine has no deck-card listeners).
    pub(crate) fn after_room_entered(&mut self) -> Flow {
        for relic in self.s.player.relics.clone() {
            if relic.melted {
                continue;
            }
            relic.kind.model().after_room_entered(self, relic.id)?;
        }
        let potions: Vec<Potion> = self.s.player.potions().collect();
        for potion in potions {
            if self.s.player.potions().any(|p| p.id == potion.id) {
                potion.kind.model().after_room_entered(self, potion.id)?;
            }
        }
        // `Murderous` and `Terminal` (modifiers come after the relics and potions).
        let flags = self.s.modifiers;
        if flags.murderous {
            let all: Vec<CreatureId> = self
                .s
                .allies
                .iter()
                .chain(&self.s.enemies)
                .copied()
                .collect();
            self.apply_power_to_all(&all, PowerKind::Strength, Decimal::from(3), None)?;
        }
        if flags.terminal {
            let player = self.s.player.creature;
            self.apply_power(player, PowerKind::Plating, Decimal::from(5), None)?;
        }
        Ok(())
    }

    /// `Hook.BeforeCombatStart` (`Hook.cs:311`, run-level): pass `BeforeCombatStart`, then
    /// `BeforeCombatStartLate`.
    #[inline]
    pub(crate) fn before_combat_start(&mut self) -> Flow {
        pass!(self, Guard::None, before_combat_start())?;
        pass!(self, Guard::None, before_combat_start_late())
    }

    /// `Hook.BeforeSideTurnStart` (`Hook.cs:1156`).
    #[inline]
    pub(crate) fn before_side_turn_start(
        &mut self,
        side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        pass!(
            self,
            Guard::Combat,
            before_side_turn_start(side, participants)
        )
    }

    /// `Hook.AfterBlockCleared` (`Hook.cs:119`).
    #[inline]
    pub(crate) fn after_block_cleared(&mut self, creature: CreatureId) -> Flow {
        pass!(self, Guard::Combat, after_block_cleared(creature))
    }

    /// `Hook.AfterPreventingBlockClear` (`Hook.cs:1044`): the preventer, if it is still a
    /// (guarded) listener.
    #[inline]
    pub(crate) fn after_preventing_block_clear(
        &mut self,
        preventer: Listener,
        creature: CreatureId,
    ) -> Flow {
        pass_to!(
            self,
            Guard::Combat,
            &[preventer],
            after_preventing_block_clear(creature)
        )
    }

    /// `Hook.AfterEnergyReset` (`Hook.cs:515`): pass `AfterEnergyReset`, then `...Late`.
    #[inline]
    pub(crate) fn after_energy_reset(&mut self) -> Flow {
        pass!(self, Guard::Combat, after_energy_reset())?;
        pass!(self, Guard::Combat, after_energy_reset_late())
    }

    /// `Hook.BeforeHandDraw` (`Hook.cs:600`): pass `BeforeHandDraw`, then `...Late`.
    #[inline]
    pub(crate) fn before_hand_draw(&mut self) -> Flow {
        pass!(self, Guard::Combat, before_hand_draw())?;
        pass!(self, Guard::Combat, before_hand_draw_late())
    }

    /// `Hook.AfterModifyingHandDraw` (`Hook.cs:751`).
    #[inline]
    pub(crate) fn after_modifying_hand_draw(&mut self, modifiers: &[Listener]) -> Flow {
        pass_to!(self, Guard::Combat, modifiers, after_modifying_hand_draw())
    }

    /// `Hook.AfterPlayerTurnStart` (`Hook.cs:894`): passes `Early`, normal, `Late`.
    #[inline]
    pub(crate) fn after_player_turn_start(&mut self) -> Flow {
        pass!(self, Guard::Combat, after_player_turn_start_early())?;
        pass!(self, Guard::Combat, after_player_turn_start())?;
        pass!(self, Guard::Combat, after_player_turn_start_late())
    }

    /// `Hook.AfterSideTurnStart` (`Hook.cs:1175`): pass normal, then `Late`.
    #[inline]
    pub(crate) fn after_side_turn_start(
        &mut self,
        side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        pass!(
            self,
            Guard::Combat,
            after_side_turn_start(side, participants)
        )?;
        pass!(
            self,
            Guard::Combat,
            after_side_turn_start_late(side, participants)
        )
    }

    /// `Hook.AfterAutoPrePlayPhaseEntered` (`Hook.cs:940`): passes `Early`, normal, `Late`.
    #[inline]
    pub(crate) fn after_auto_pre_play_phase_entered(&mut self) -> Flow {
        pass!(
            self,
            Guard::Combat,
            after_auto_pre_play_phase_entered_early()
        )?;
        pass!(self, Guard::Combat, after_auto_pre_play_phase_entered())?;
        pass!(
            self,
            Guard::Combat,
            after_auto_pre_play_phase_entered_late()
        )
    }

    /// `Hook.AfterHandEmptied` (`Hook.cs:623`).
    #[inline]
    pub(crate) fn after_hand_emptied(&mut self) -> Flow {
        pass!(self, Guard::Combat, after_hand_emptied())
    }

    /// `Hook.AfterAutoPostPlayPhaseEntered` (`Hook.cs:922`).
    #[inline]
    pub(crate) fn after_auto_post_play_phase_entered(&mut self) -> Flow {
        pass!(self, Guard::Combat, after_auto_post_play_phase_entered())
    }

    /// `Hook.BeforeSideTurnEnd` (`Hook.cs:1244`): passes `VeryEarly`, `Early`, normal.
    #[inline]
    pub(crate) fn before_side_turn_end(&mut self, side: Side, participants: &[CreatureId]) -> Flow {
        pass!(
            self,
            Guard::Combat,
            before_side_turn_end_very_early(side, participants)
        )?;
        pass!(
            self,
            Guard::Combat,
            before_side_turn_end_early(side, participants)
        )?;
        pass!(
            self,
            Guard::Combat,
            before_side_turn_end(side, participants)
        )
    }

    /// `Hook.BeforeFlush` (`Hook.cs:544`): pass `BeforeFlush`, then `...Late`.
    #[inline]
    pub(crate) fn before_flush(&mut self) -> Flow {
        pass!(self, Guard::Combat, before_flush())?;
        pass!(self, Guard::Combat, before_flush_late())
    }

    /// `Hook.AfterTakingExtraTurn` (`Hook.cs:1232`, guarded).
    #[inline]
    pub(crate) fn after_taking_extra_turn(&mut self) -> Flow {
        pass!(self, Guard::Combat, after_taking_extra_turn())
    }

    /// `Hook.AfterFlush` (`Hook.cs:572`).
    #[inline]
    pub(crate) fn after_flush(&mut self, flushed: &[CardId], retained: &[CardId]) -> Flow {
        pass!(self, Guard::Combat, after_flush(flushed, retained))
    }

    /// `Hook.AfterSideTurnEnd` (`Hook.cs:1279`): pass normal, then `Late`.
    #[inline]
    pub(crate) fn after_side_turn_end(&mut self, side: Side, participants: &[CreatureId]) -> Flow {
        pass!(self, Guard::Combat, after_side_turn_end(side, participants))?;
        pass!(
            self,
            Guard::Combat,
            after_side_turn_end_late(side, participants)
        )
    }

    /// `Hook.AfterEnergySpent` (`Hook.cs:532`).
    #[inline]
    pub(crate) fn after_energy_spent(&mut self, card: CardId, amount: i32) -> Flow {
        pass!(self, Guard::Combat, after_energy_spent(card, amount))
    }

    /// `Hook.AfterStarsGained` (`Hook.cs:1192`, guarded).
    #[inline]
    pub(crate) fn after_stars_gained(&mut self, amount: i32) -> Flow {
        pass!(self, Guard::Combat, after_stars_gained(amount))
    }

    /// `Hook.AfterStarsSpent` (`Hook.cs:1204`, guarded).
    #[inline]
    pub(crate) fn after_stars_spent(&mut self, amount: i32) -> Flow {
        pass!(self, Guard::Combat, after_stars_spent(amount))
    }

    /// `Hook.AfterForge` (`Hook.cs:586`, guarded).
    #[inline]
    pub(crate) fn after_forge(&mut self, amount: Decimal, source: Option<Listener>) -> Flow {
        pass!(self, Guard::Combat, after_forge(amount, source))
    }

    /// `AfterModifyingCardPlayResultLocation` on each modifier, in order, unguarded
    /// (`CardModel.cs:1883-1886` calls the models directly).
    #[inline]
    pub(crate) fn after_modifying_card_play_result_location(
        &mut self,
        card: CardId,
        location: ResultLocation,
        modifiers: &[Listener],
    ) -> Flow {
        pass_to!(
            self,
            Guard::None,
            modifiers,
            after_modifying_card_play_result_location(card, location)
        )
    }

    /// `Hook.AfterModifyingCardPlayCount` (`Hook.cs:676`).
    #[inline]
    pub(crate) fn after_modifying_card_play_count(
        &mut self,
        card: CardId,
        modifiers: &[Listener],
    ) -> Flow {
        pass_to!(
            self,
            Guard::Combat,
            modifiers,
            after_modifying_card_play_count(card)
        )
    }

    /// `Hook.BeforeCardPlayed` (`Hook.cs:263`, guarded).
    #[inline]
    pub(crate) fn before_card_played(&mut self, play: &CardPlay) -> Flow {
        pass!(self, Guard::Combat, before_card_played(play))
    }

    /// `Hook.AfterCardPlayed` (`Hook.cs:278`): unguarded, pass normal then `Late`.
    #[inline]
    pub(crate) fn after_card_played(&mut self, play: &CardPlay) -> Flow {
        pass!(self, Guard::None, after_card_played(play))?;
        pass!(self, Guard::None, after_card_played_late(play))
    }

    /// `Hook.AfterCreatureAddedToCombat` (`Hook.cs:374`): over the combat listeners, unguarded.
    #[inline]
    pub(crate) fn after_creature_added_to_combat(&mut self, creature: CreatureId) -> Flow {
        pass!(self, Guard::None, after_creature_added_to_combat(creature))
    }

    /// `Hook.AfterCardDiscarded` (`Hook.cs:186`).
    #[inline]
    pub(crate) fn after_card_discarded(&mut self, card: CardId) -> Flow {
        pass!(self, Guard::Combat, after_card_discarded(card))
    }

    /// `Hook.AfterCardEnteredCombat` (`Hook.cs:223`).
    #[inline]
    pub(crate) fn after_card_entered_combat(&mut self, card: CardId) -> Flow {
        pass!(self, Guard::Combat, after_card_entered_combat(card))
    }

    /// Whether no listener overrides [`Engine::after_card_changed_piles`]'s passes, so that it
    /// visits nobody. A pass that visits nobody changes nothing, so this then also holds for
    /// every later card of the same move.
    #[inline]
    pub(crate) fn lacks_after_card_changed_piles(&self) -> bool {
        self.lacks(Hook::after_card_changed_piles)
            && self.lacks(Hook::after_card_changed_piles_late)
    }

    /// `Hook.AfterCardChangedPiles` (`Hook.cs:167`, run-level): pass normal, then `Late`.
    #[inline]
    pub(crate) fn after_card_changed_piles(
        &mut self,
        card: CardId,
        old_pile: Option<PileType>,
    ) -> Flow {
        pass!(self, Guard::None, after_card_changed_piles(card, old_pile))?;
        pass!(
            self,
            Guard::None,
            after_card_changed_piles_late(card, old_pile)
        )
    }

    /// `Hook.AfterCardGeneratedForCombat` (`Hook.cs:251`).
    #[inline]
    pub(crate) fn after_card_generated_for_combat(
        &mut self,
        card: CardId,
        creator: Option<CreatureId>,
    ) -> Flow {
        pass!(
            self,
            Guard::Combat,
            after_card_generated_for_combat(card, creator)
        )
    }

    /// `Hook.AfterCardDrawn` (`Hook.cs:202`): pass `AfterCardDrawnEarly`, then normal.
    #[inline]
    pub(crate) fn after_card_drawn(&mut self, card: CardId, from_hand_draw: bool) -> Flow {
        pass!(
            self,
            Guard::Combat,
            after_card_drawn_early(card, from_hand_draw)
        )?;
        pass!(self, Guard::Combat, after_card_drawn(card, from_hand_draw))
    }

    /// `Hook.AfterPreventingDraw` (`Hook.cs:1068`): the preventer, if still a listener.
    #[inline]
    pub(crate) fn after_preventing_draw(&mut self, preventer: Listener) -> Flow {
        pass_to!(self, Guard::Combat, &[preventer], after_preventing_draw())
    }

    /// `Hook.AfterShuffle` (`Hook.cs:1142`).
    #[inline]
    pub(crate) fn after_shuffle(&mut self) -> Flow {
        pass!(self, Guard::Combat, after_shuffle())
    }

    /// `Hook.AfterCardExhausted` (`Hook.cs:237`).
    #[inline]
    pub(crate) fn after_card_exhausted(&mut self, card: CardId, caused_by_ethereal: bool) -> Flow {
        pass!(
            self,
            Guard::Combat,
            after_card_exhausted(card, caused_by_ethereal)
        )
    }

    /// `Hook.BeforeAttack` (`Hook.cs:80`).
    #[inline]
    pub(crate) fn before_attack(&mut self, attack: &Attack) -> Flow {
        pass!(self, Guard::Combat, before_attack(attack))
    }

    /// `Hook.AfterAttack` (`Hook.cs:92`).
    #[inline]
    pub(crate) fn after_attack(&mut self, attack: &Attack, results: &[DamageResult]) -> Flow {
        pass!(self, Guard::Combat, after_attack(attack, results))
    }

    /// `Hook.AfterModifyingDamageAmount` (`Hook.cs:706`, run-level).
    #[inline]
    pub(crate) fn after_modifying_damage_amount(
        &mut self,
        card: Option<CardId>,
        modifiers: &[Listener],
    ) -> Flow {
        pass_to!(
            self,
            Guard::None,
            modifiers,
            after_modifying_damage_amount(card)
        )
    }

    /// `Hook.BeforeDamageReceived` (`Hook.cs:415`, run-level).
    #[inline]
    pub(crate) fn before_damage_received(&mut self, query: HpLossQuery, amount: Decimal) -> Flow {
        pass!(self, Guard::None, before_damage_received(query, amount))
    }

    /// `Hook.AfterModifyingHpLostBeforeOsty` / `...AfterOsty` (`Hook.cs:766,781`, run-level).
    #[inline]
    pub(crate) fn after_modifying_hp_lost(
        &mut self,
        phase: HpLossPhase,
        modifiers: &[Listener],
    ) -> Flow {
        match phase {
            HpLossPhase::BeforeOsty => pass_to!(
                self,
                Guard::None,
                modifiers,
                after_modifying_hp_lost_before_osty()
            ),
            HpLossPhase::AfterOsty => pass_to!(
                self,
                Guard::None,
                modifiers,
                after_modifying_hp_lost_after_osty()
            ),
        }
    }

    /// `Hook.AfterBlockBroken` (`Hook.cs:107`, unguarded).
    #[inline]
    pub(crate) fn after_block_broken(
        &mut self,
        target: CreatureId,
        breaker: Option<CreatureId>,
    ) -> Flow {
        pass!(self, Guard::None, after_block_broken(target, breaker))
    }

    /// `Hook.AfterCurrentHpChanged` (`Hook.cs:386`, run-level).
    #[inline]
    pub(crate) fn after_current_hp_changed(
        &mut self,
        creature: CreatureId,
        delta: Decimal,
    ) -> Flow {
        pass!(self, Guard::None, after_current_hp_changed(creature, delta))
    }

    /// `Hook.AfterDamageGiven` (`Hook.cs:401`, unguarded).
    #[inline]
    pub(crate) fn after_damage_given(&mut self, hit: DamageEvent) -> Flow {
        pass!(self, Guard::None, after_damage_given(hit))
    }

    /// `Hook.AfterDamageReceived` (`Hook.cs:429`, run-level): pass normal, then `Late`.
    #[inline]
    pub(crate) fn after_damage_received(&mut self, hit: DamageEvent) -> Flow {
        pass!(self, Guard::None, after_damage_received(hit))?;
        pass!(self, Guard::None, after_damage_received_late(hit))
    }

    /// `Hook.BeforeDeath` (`Hook.cs:450`, run-level).
    #[inline]
    pub(crate) fn before_death(&mut self, creature: CreatureId) -> Flow {
        pass!(self, Guard::None, before_death(creature))
    }

    /// `Hook.AfterDeath` (`Hook.cs:462`, run-level).
    #[inline]
    pub(crate) fn after_death(
        &mut self,
        creature: CreatureId,
        was_removal_prevented: bool,
    ) -> Flow {
        pass!(
            self,
            Guard::None,
            after_death(creature, was_removal_prevented)
        )
    }

    /// `Hook.AfterPreventingDeath` (`Hook.cs:1056`, run-level): the preventer, if still a
    /// listener.
    #[inline]
    pub(crate) fn after_preventing_death(
        &mut self,
        preventer: Listener,
        creature: CreatureId,
    ) -> Flow {
        pass_to!(
            self,
            Guard::None,
            &[preventer],
            after_preventing_death(creature)
        )
    }

    /// `Hook.AfterDiedToDoom` (`Hook.cs:496`, unguarded), called by `DoomPower`.
    #[inline]
    pub(crate) fn after_died_to_doom(&mut self, creatures: &[CreatureId]) -> Flow {
        pass!(self, Guard::None, after_died_to_doom(creatures))
    }

    /// `Hook.BeforeBlockGained` (`Hook.cs:131`).
    #[inline]
    pub(crate) fn before_block_gained(&mut self, query: BlockQuery, amount: Decimal) -> Flow {
        pass!(self, Guard::Combat, before_block_gained(query, amount))
    }

    /// `Hook.AfterModifyingBlockAmount` (`Hook.cs:661`).
    #[inline]
    pub(crate) fn after_modifying_block_amount(
        &mut self,
        amount: Decimal,
        card: Option<CardId>,
        play: Option<&CardPlay>,
        modifiers: &[Listener],
    ) -> Flow {
        pass_to!(
            self,
            Guard::Combat,
            modifiers,
            after_modifying_block_amount(amount, card, play)
        )
    }

    /// `Hook.AfterBlockGained` (`Hook.cs:143`).
    #[inline]
    pub(crate) fn after_block_gained(&mut self, query: BlockQuery, amount: Decimal) -> Flow {
        pass!(self, Guard::Combat, after_block_gained(query, amount))
    }

    /// `Hook.BeforePowerAmountChanged` (`Hook.cs:1020`).
    #[inline]
    pub(crate) fn before_power_amount_changed(
        &mut self,
        power: PowerQuery,
        amount: Decimal,
    ) -> Flow {
        pass!(
            self,
            Guard::Combat,
            before_power_amount_changed(power, amount)
        )
    }

    /// `Hook.AfterModifyingPowerAmountGiven` (`Hook.cs:811`).
    #[inline]
    pub(crate) fn after_modifying_power_amount_given(
        &mut self,
        power: PowerQuery,
        modifiers: &[Listener],
    ) -> Flow {
        pass_to!(
            self,
            Guard::Combat,
            modifiers,
            after_modifying_power_amount_given(power)
        )
    }

    /// `Hook.AfterModifyingPowerAmountReceived` (`Hook.cs:826`).
    #[inline]
    pub(crate) fn after_modifying_power_amount_received(
        &mut self,
        power: PowerQuery,
        modifiers: &[Listener],
    ) -> Flow {
        pass_to!(
            self,
            Guard::Combat,
            modifiers,
            after_modifying_power_amount_received(power)
        )
    }

    /// `Hook.AfterPowerAmountChanged` (`Hook.cs:1032`).
    #[inline]
    pub(crate) fn after_power_amount_changed(
        &mut self,
        power: PowerRef,
        amount: Decimal,
        applier: Option<CreatureId>,
    ) -> Flow {
        pass!(
            self,
            Guard::Combat,
            after_power_amount_changed(power, amount, applier)
        )
    }

    /// `Hook.AfterModifyingOrbPassiveTriggerCount` (`Hook.cs:796-805`): the modifiers only.
    #[inline]
    pub(crate) fn after_modifying_orb_passive_trigger_count(
        &mut self,
        orb: Orb,
        modifiers: &[Listener],
    ) -> Flow {
        pass_to!(
            self,
            Guard::Combat,
            modifiers,
            after_modifying_orb_passive_trigger_count(orb)
        )
    }

    /// `Hook.AfterOrbChanneled` (`Hook.cs:856-865`).
    #[inline]
    pub(crate) fn after_orb_channeled(&mut self, orb: Orb) -> Flow {
        pass!(self, Guard::Combat, after_orb_channeled(orb))
    }

    /// `Hook.AfterOrbEvoked` (`Hook.cs:870-877`).
    #[inline]
    pub(crate) fn after_orb_evoked(&mut self, orb: Orb, targets: &[CreatureId]) -> Flow {
        pass!(self, Guard::Combat, after_orb_evoked(orb, targets))
    }

    /// `Hook.BeforePotionUsed` (`Hook.cs:996`, run-level).
    #[inline]
    pub(crate) fn before_potion_used(
        &mut self,
        potion: Potion,
        target: Option<CreatureId>,
    ) -> Flow {
        pass!(self, Guard::None, before_potion_used(potion, target))
    }

    /// `Hook.AfterPotionUsed` (`Hook.cs:1008`, run-level).
    #[inline]
    pub(crate) fn after_potion_used(&mut self, potion: Potion, target: Option<CreatureId>) -> Flow {
        pass!(self, Guard::None, after_potion_used(potion, target))
    }

    /// `Hook.AfterPotionProcured` (`Hook.cs:984`, run-level), called by `PotionCmd.TryToProcure`
    /// (`PotionCmd.cs:46`).
    #[inline]
    pub(crate) fn after_potion_procured(&mut self, potion: Potion) -> Flow {
        pass!(self, Guard::None, after_potion_procured(potion))
    }

    /// `Hook.AfterPotionDiscarded` (`Hook.cs:972`, run-level), called by `PotionCmd.Discard`
    /// (`PotionCmd.cs:59`).
    #[inline]
    pub(crate) fn after_potion_discarded(&mut self, potion: Potion) -> Flow {
        pass!(self, Guard::None, after_potion_discarded(potion))
    }

    /// `Hook.AfterCombatEnd` (`Hook.cs:328`, run-level), called by `EndCombatInternal` on a
    /// win only.
    #[inline]
    pub(crate) fn after_combat_end(&mut self) -> Flow {
        pass!(self, Guard::None, after_combat_end())
    }

    /// `Hook.AfterCombatVictory` (`Hook.cs:352`, run-level): pass `Early`, then normal.
    #[inline]
    pub(crate) fn after_combat_victory(&mut self) -> Flow {
        pass!(self, Guard::None, after_combat_victory_early())?;
        pass!(self, Guard::None, after_combat_victory())
    }
}
