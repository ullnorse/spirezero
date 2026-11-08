//! `NeurosurgePower` (`Models.Powers/NeurosurgePower.cs`): at the start of its owner's side's
//! turn, applies `Amount` Doom to the owner.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Neurosurge;

impl PowerModel for Neurosurge {
    fn name(&self) -> &'static str {
        "Neurosurge"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

#[sts2_macros::hooks]
impl Hooks for Neurosurge {
    type Me = PowerRef;

    fn after_side_turn_start(
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
        eng.apply_power(me.owner, PowerKind::Doom, amount, Some(me.owner))
    }
}
