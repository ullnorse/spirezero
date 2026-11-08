//! `DemonFormPower` (`Models.Powers/DemonFormPower.cs`): the owner gains `Amount` Strength at the
//! start of its side's turn. The VFX has no game effect.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct DemonForm;

impl PowerModel for DemonForm {
    fn name(&self) -> &'static str {
        "Demon Form"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for DemonForm {
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
            eng.apply_power(me.owner, PowerKind::Strength, amount, Some(me.owner))?;
        }
        Ok(())
    }
}
