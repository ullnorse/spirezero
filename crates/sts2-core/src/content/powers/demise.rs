//! `DemisePower` (`Models.Powers/DemisePower.cs`): at the end of its side's turn the owner takes
//! `Amount` unblockable, unpowered damage from no one.

use crate::combat::ValueProps;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Demise;

impl PowerModel for Demise {
    fn name(&self) -> &'static str {
        "Demise"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

#[sts2_macros::hooks]
impl Hooks for Demise {
    type Me = PowerRef;

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) {
            let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
            eng.damage(
                &[me.owner],
                amount,
                ValueProps::UNBLOCKABLE.with(ValueProps::UNPOWERED),
                None,
                None,
            )?;
        }
        Ok(())
    }
}
