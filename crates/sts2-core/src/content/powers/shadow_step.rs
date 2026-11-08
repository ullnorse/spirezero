//! `ShadowStepPower` (`Models.Powers/ShadowStepPower.cs`): a buff (`Counter`); when its owner's
//! side's turn starts it becomes `DoubleDamagePower` with the same amount.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct ShadowStep;

impl PowerModel for ShadowStep {
    fn name(&self) -> &'static str {
        "Shadow Step"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for ShadowStep {
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
            eng.apply_power(me.owner, PowerKind::DoubleDamage, amount, Some(me.owner))?;
            eng.remove_power(me.owner, me.id)?;
        }
        Ok(())
    }
}
