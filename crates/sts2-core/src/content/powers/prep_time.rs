//! `PrepTimePower` (`Models.Powers/PrepTimePower.cs`): at the start of the owner's side's turn,
//! apply `Amount` Vigor to the owner.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct PrepTime;

impl PowerModel for PrepTime {
    fn name(&self) -> &'static str {
        "Prep Time"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for PrepTime {
    type Me = PowerRef;

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) {
            let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
            eng.apply_power(me.owner, PowerKind::Vigor, amount, Some(me.owner))?;
        }
        Ok(())
    }
}
