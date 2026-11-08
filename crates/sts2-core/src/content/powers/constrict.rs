//! `ConstrictPower` (`Models.Powers/ConstrictPower.cs`): the owner takes `Amount` unpowered
//! damage at the end of its side's turn; removed when its applier dies.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side, ValueProps};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Constrict;

impl PowerModel for Constrict {
    fn name(&self) -> &'static str {
        "Constrict"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

#[sts2_macros::hooks]
impl Hooks for Constrict {
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
                ValueProps::UNPOWERED,
                Some(me.owner),
                None,
            )?;
        }
        Ok(())
    }

    fn after_death(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        creature: CreatureId,
        was_removal_prevented: bool,
    ) -> Flow {
        if !was_removal_prevented && Some(creature) == eng.s.power(me.owner, me.id).applier {
            eng.remove_power(me.owner, me.id)?;
        }
        Ok(())
    }
}
