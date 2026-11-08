//! `ReflectPower` (`Models.Powers/ReflectPower.cs`): a buff (`Counter`); powered attack damage
//! the owner's block absorbed is dealt back to the attacker (unpowered). Loses one stack at the
//! start of the owner's side's turn.

use crate::combat::hooks::{DamageEvent, Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side, ValueProps};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Reflect;

impl PowerModel for Reflect {
    fn name(&self) -> &'static str {
        "Reflect"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Reflect {
    type Me = PowerRef;

    fn after_damage_received(&self, eng: &mut Engine<'_>, me: PowerRef, hit: DamageEvent) -> Flow {
        if hit.target != me.owner || hit.result.blocked <= 0 || !hit.props.is_powered_attack() {
            return Ok(());
        }
        let Some(dealer) = hit.dealer else {
            return Ok(());
        };
        eng.damage(
            &[dealer],
            Decimal::from(hit.result.blocked),
            ValueProps::UNPOWERED,
            Some(me.owner),
            None,
        )?;
        Ok(())
    }

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) {
            eng.decrement_power(me)?;
        }
        Ok(())
    }
}
