//! `MonarchsGazePower` (`Models.Powers/MonarchsGazePower.cs`): a counter buff; whenever its owner
//! deals powered attack damage, the target gets `Amount` `MonarchsGazeStrengthDownPower`.

use crate::combat::hooks::{DamageEvent, Hooks, PowerRef};
use crate::combat::{Engine, Flow};
use crate::content::powers::{PowerKind, PowerModel, PowerType};

pub struct MonarchsGaze;

impl PowerModel for MonarchsGaze {
    fn name(&self) -> &'static str {
        "Monarch's Gaze"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for MonarchsGaze {
    type Me = PowerRef;

    fn after_damage_given(&self, eng: &mut Engine<'_>, me: PowerRef, hit: DamageEvent) -> Flow {
        if hit.dealer == Some(me.owner) && hit.props.is_powered_attack() {
            let amount = eng.s.power(me.owner, me.id).amount;
            eng.apply_power(
                hit.target,
                PowerKind::MonarchsGazeStrengthDown,
                amount.into(),
                Some(me.owner),
            )?;
        }
        Ok(())
    }
}
