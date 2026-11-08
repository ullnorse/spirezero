//! `VeilpiercerPower` (`Models.Powers/VeilpiercerPower.cs`): the owner's Ethereal cards in hand
//! (or being played) cost 0; each Ethereal play from there uses up one `Amount`.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::state::PileType;
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow};
use crate::content::cards::CardKeyword;
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Veilpiercer;

impl PowerModel for Veilpiercer {
    fn name(&self) -> &'static str {
        "Veilpiercer"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

/// An Ethereal card of the owner in the Hand or Play pile.
fn is_free_ethereal(s: &CombatState, me: PowerRef, card: CardId) -> bool {
    // `card.Owner.Creature == Owner`: every card belongs to the player.
    s.player.creature == me.owner
        && s.has_keyword(card, CardKeyword::Ethereal)
        && matches!(s.pile_of(card), Some(PileType::Hand | PileType::Play))
}

#[sts2_macros::hooks]
impl Hooks for Veilpiercer {
    type Me = PowerRef;

    fn try_modify_energy_cost_in_combat_late(
        &self,
        s: &CombatState,
        me: PowerRef,
        card: CardId,
        _cost: Decimal,
    ) -> Option<Decimal> {
        is_free_ethereal(s, me, card).then_some(Decimal::ZERO)
    }

    fn before_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        if is_free_ethereal(eng.s, me, play.card) {
            eng.decrement_power(me)?;
        }
        Ok(())
    }
}
