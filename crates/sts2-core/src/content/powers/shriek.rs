//! `ShriekPower` (`Models.Powers/ShriekPower.cs`): a debuff (`Counter`, `AllowNegative`) on
//! `TerrorEel` itself. When a hit takes HP off its owner and leaves it at or below `Amount` HP, the
//! owner is stunned into its `TERROR_MOVE` and the power is removed.

use crate::combat::hooks::{DamageEvent, Hooks, PowerRef};
use crate::combat::{Engine, Flow};
use crate::content::monsters::{self, TerrorEel};
use crate::content::powers::{PowerModel, PowerType};

pub struct Shriek;

impl PowerModel for Shriek {
    fn name(&self) -> &'static str {
        "Shriek"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }

    fn allows_negative(&self) -> bool {
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for Shriek {
    type Me = PowerRef;

    fn after_damage_received(&self, eng: &mut Engine<'_>, me: PowerRef, hit: DamageEvent) -> Flow {
        let owner = eng.s.creature(me.owner);
        if hit.target != me.owner
            || hit.result.unblocked <= 0
            || owner.hp > eng.s.power(me.owner, me.id).amount
        {
            return Ok(());
        }
        monsters::stun(eng, me.owner, Some(TerrorEel::TERROR));
        eng.remove_power(me.owner, me.id)
    }
}
