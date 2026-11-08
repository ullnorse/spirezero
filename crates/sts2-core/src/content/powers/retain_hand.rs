//! `RetainHandPower` (`Models.Powers/RetainHandPower.cs`): the owner's hand is not flushed at the
//! end of its turn; loses one stack after each of its side's turns.

use crate::combat::CombatState;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerModel, PowerType};

pub struct RetainHand;

impl PowerModel for RetainHand {
    fn name(&self) -> &'static str {
        "Retain Hand"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for RetainHand {
    type Me = PowerRef;

    // The owner is always the player (`player != Owner.Player` is never true).
    fn should_flush(&self, _s: &CombatState, _me: PowerRef) -> bool {
        false
    }

    fn after_side_turn_end(
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
