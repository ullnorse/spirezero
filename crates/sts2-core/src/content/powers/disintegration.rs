//! `DisintegrationPower` (`Models.Powers/DisintegrationPower.cs`): a debuff (`Counter`) from the
//! `KnowledgeDemon`'s Curse of Knowledge. After each turn end of its owner's side (late), the
//! owner takes `Amount` unpowered damage from itself.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side, ValueProps};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

/// Damage at the end of every turn.
pub struct Disintegration;

impl PowerModel for Disintegration {
    fn name(&self) -> &'static str {
        "Disintegration"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

#[sts2_macros::hooks]
impl Hooks for Disintegration {
    type Me = PowerRef;

    fn after_side_turn_end_late(
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
        eng.damage(
            &[me.owner],
            amount,
            ValueProps::UNPOWERED,
            Some(me.owner),
            None,
        )?;
        Ok(())
    }
}
