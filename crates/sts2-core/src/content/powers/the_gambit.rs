//! `TheGambitPower` (`Models.Powers/TheGambitPower.cs`): a debuff; when the owner takes unblocked
//! powered attack damage it removes itself and kills the owner.

use crate::combat::Engine;
use crate::combat::Flow;
use crate::combat::hooks::{DamageEvent, Hooks, PowerRef};
use crate::content::powers::{PowerModel, PowerType};

pub struct TheGambit;

impl PowerModel for TheGambit {
    fn name(&self) -> &'static str {
        "The Gambit"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

#[sts2_macros::hooks]
impl Hooks for TheGambit {
    type Me = PowerRef;

    fn after_damage_received(&self, eng: &mut Engine<'_>, me: PowerRef, hit: DamageEvent) -> Flow {
        if hit.target == me.owner && hit.props.is_powered_attack() && hit.result.unblocked > 0 {
            eng.remove_power(me.owner, me.id)?;
            eng.kill(&[me.owner])?;
        }
        Ok(())
    }
}
