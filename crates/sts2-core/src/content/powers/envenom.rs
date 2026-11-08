//! `EnvenomPower` (`Models.Powers/EnvenomPower.cs`): when the owner's powered attack deals
//! unblocked damage, the target gets `Amount` Poison.

use crate::combat::hooks::{DamageEvent, Hooks, PowerRef};
use crate::combat::{Engine, Flow};
use crate::content::powers::{PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Envenom;

impl PowerModel for Envenom {
    fn name(&self) -> &'static str {
        "Envenom"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Envenom {
    type Me = PowerRef;

    fn after_damage_given(&self, eng: &mut Engine<'_>, me: PowerRef, hit: DamageEvent) -> Flow {
        if hit.dealer == Some(me.owner) && hit.props.is_powered_attack() && hit.result.unblocked > 0
        {
            let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
            eng.apply_power(hit.target, PowerKind::Poison, amount, Some(me.owner))?;
        }
        Ok(())
    }
}
