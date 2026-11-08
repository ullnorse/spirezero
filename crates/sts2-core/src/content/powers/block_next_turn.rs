//! `BlockNextTurnPower` (`Models.Powers/BlockNextTurnPower.cs`): when the owner's Block is
//! cleared at its turn start, gain `Amount` unpowered Block and remove itself.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, ValueProps};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct BlockNextTurn;

impl PowerModel for BlockNextTurn {
    fn name(&self) -> &'static str {
        "Block Next Turn"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for BlockNextTurn {
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
