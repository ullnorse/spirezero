//! `BiasedCognitionPower` (`Models.Powers/BiasedCognitionPower.cs`): at the start of its side's
//! turn the owner loses `Amount` Focus (a debuff on the player; the Focus it took away is the
//! card's).

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct BiasedCognition;

impl PowerModel for BiasedCognition {
    fn name(&self) -> &'static str {
        "Biased Cognition"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

#[sts2_macros::hooks]
impl Hooks for BiasedCognition {
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
                PowerKind::Focus,
                Decimal::from(-amount),
                Some(me.owner),
            )?;
        }
        Ok(())
    }
}
