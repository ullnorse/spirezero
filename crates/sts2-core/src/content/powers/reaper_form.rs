//! `ReaperFormPower` (`Models.Powers/ReaperFormPower.cs`): damage the owner or the owner's pet
//! deals with a powered attack applies Doom to the target equal to the total damage times
//! `Amount`. The VFX has no game effect.

use crate::combat::hooks::{DamageEvent, Hooks, PowerRef};
use crate::combat::{Engine, Flow};
use crate::content::powers::{PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct ReaperForm;

impl PowerModel for ReaperForm {
    fn name(&self) -> &'static str {
        "Reaper Form"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for ReaperForm {
    type Me = PowerRef;

    fn after_damage_given(&self, eng: &mut Engine<'_>, me: PowerRef, hit: DamageEvent) -> Flow {
        let Some(dealer) = hit.dealer else {
            return Ok(());
        };
        let from_owner = dealer == me.owner || eng.s.creature(dealer).pet_owner == Some(me.owner);
        // `TotalDamage`: `BlockedDamage + UnblockedDamage`.
        let total = hit.result.blocked + hit.result.unblocked;
        if !from_owner || !hit.props.is_powered_attack() || total <= 0 {
            return Ok(());
        }
        let amount = Decimal::from(total) * Decimal::from(eng.s.power(me.owner, me.id).amount);
        eng.apply_power(hit.target, PowerKind::Doom, amount, Some(me.owner))
    }
}
