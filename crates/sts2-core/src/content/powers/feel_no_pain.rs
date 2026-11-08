//! `FeelNoPainPower` (`Models.Powers/FeelNoPainPower.cs`): a buff (`Counter`); the owner gains
//! `Amount` unpowered block whenever one of their cards is exhausted.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, Engine, Flow, ValueProps};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct FeelNoPain;

impl PowerModel for FeelNoPain {
    fn name(&self) -> &'static str {
        "Feel No Pain"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for FeelNoPain {
    type Me = PowerRef;

    fn after_card_exhausted(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _card: CardId,
        _caused_by_ethereal: bool,
    ) -> Flow {
        // `card.Owner.Creature == Owner`: every card belongs to the player.
        if eng.s.player.creature == me.owner {
            let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
            eng.gain_block(me.owner, amount, ValueProps::UNPOWERED, None)?;
        }
        Ok(())
    }
}
