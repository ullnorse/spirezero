//! `SelfFormingClayPower` (`Models.Powers/SelfFormingClayPower.cs`): when the owner's Block is
//! cleared (its next turn), it gains `Amount` unpowered Block and the power is removed.

use crate::combat::ValueProps;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct SelfFormingClay;

impl PowerModel for SelfFormingClay {
    fn name(&self) -> &'static str {
        "Self-Forming Clay"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for SelfFormingClay {
    type Me = PowerRef;

    fn after_block_cleared(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        creature: CreatureId,
    ) -> Flow {
        if creature == me.owner {
            let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
            eng.gain_block(me.owner, amount, ValueProps::UNPOWERED, None)?;
            eng.remove_power(me.owner, me.id)?;
        }
        Ok(())
    }
}
