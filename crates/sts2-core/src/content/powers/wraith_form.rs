//! `WraithFormPower` (`Models.Powers/WraithFormPower.cs`): a debuff; at every turn start of the
//! owner's side the owner loses `Amount` Dexterity.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct WraithForm;

impl PowerModel for WraithForm {
    fn name(&self) -> &'static str {
        "Wraith Form"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

#[sts2_macros::hooks]
impl Hooks for WraithForm {
    type Me = PowerRef;

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) {
            let amount = eng.s.power(me.owner, me.id).amount;
            eng.apply_power(
                me.owner,
                PowerKind::Dexterity,
                Decimal::from(-amount),
                Some(me.owner),
            )?;
        }
        Ok(())
    }
}
