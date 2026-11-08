//! `TerritorialPower` (`Models.Powers/TerritorialPower.cs`): the owner gains `Amount` Strength at
//! the end of its side's turn.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Territorial;

impl PowerModel for Territorial {
    fn name(&self) -> &'static str {
        "Territorial"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Territorial {
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
            eng.apply_power(me.owner, PowerKind::Strength, amount, Some(me.owner))?;
        }
        Ok(())
    }
}
