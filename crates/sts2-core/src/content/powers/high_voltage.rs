//! `HighVoltagePower` (`Models.Powers/HighVoltagePower.cs`): a buff (`Counter`) on `Zapbot`. At
//! the end of each turn of its side the owner gains `Amount` Strength.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct HighVoltage;

impl PowerModel for HighVoltage {
    fn name(&self) -> &'static str {
        "High Voltage"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for HighVoltage {
    type Me = PowerRef;

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if !participants.contains(&me.owner) {
            return Ok(());
        }
        let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
        eng.apply_power(me.owner, PowerKind::Strength, amount, Some(me.owner))
    }
}
