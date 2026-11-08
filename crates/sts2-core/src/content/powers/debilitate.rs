//! `DebilitatePower` (`Models.Powers/DebilitatePower.cs`): doubles the effect of the owner's
//! Vulnerable and Weak (read by `VulnerablePower` and `WeakPower`); loses 1 after the owner's
//! side's turn.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerModel, PowerType};

pub struct Debilitate;

impl PowerModel for Debilitate {
    fn name(&self) -> &'static str {
        "Debilitate"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

#[sts2_macros::hooks]
impl Hooks for Debilitate {
    type Me = PowerRef;

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
