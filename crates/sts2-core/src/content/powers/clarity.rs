//! `ClarityPower` (`Models.Powers/ClarityPower.cs`): the owner's hand draw is one card larger;
//! loses one stack when its side's turn starts.

use crate::combat::CombatState;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Clarity;

impl PowerModel for Clarity {
    fn name(&self) -> &'static str {
        "Clarity"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Clarity {
    type Me = PowerRef;

    // The owner is always the player (`player != Owner.Player` is never true).
    fn modify_hand_draw(&self, _s: &CombatState, _me: PowerRef, count: Decimal) -> Decimal {
        count + Decimal::ONE
    }

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) {
            eng.decrement_power(me)?;
        }
        Ok(())
    }
}
