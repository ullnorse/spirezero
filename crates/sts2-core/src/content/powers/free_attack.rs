//! `FreeAttackPower` (`Models.Powers/FreeAttackPower.cs`): a buff (`Counter`); the owner's Attacks
//! in the hand or being played cost 0, and each Attack played from the hand uses up one stack.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow, PileType};
use crate::content::cards::CardType;
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct FreeAttack;

/// The card is an Attack in the hand or the Play pile.
fn is_free(s: &CombatState, card: CardId) -> bool {
    s.card(card).card_type() == CardType::Attack
        && matches!(s.pile_of(card), Some(PileType::Hand | PileType::Play))
}

impl PowerModel for FreeAttack {
    fn name(&self) -> &'static str {
        "Free Attack"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for FreeAttack {
    type Me = PowerRef;

    fn try_modify_energy_cost_in_combat_late(
        &self,
        s: &CombatState,
        me: PowerRef,
        card: CardId,
        _cost: Decimal,
    ) -> Option<Decimal> {
        // `card.Owner.Creature != Owner`: every card belongs to the player.
        (s.player.creature == me.owner && is_free(s, card)).then_some(Decimal::ZERO)
    }

    fn before_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        if eng.s.player.creature == me.owner && is_free(eng.s, play.card) {
            eng.decrement_power(me)?;
        }
        Ok(())
    }
}
