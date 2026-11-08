//! `HardToKillPower` (`Models.Powers/HardToKillPower.cs`): caps every hit the owner takes at
//! `Amount`. `AfterModifyingDamageAmount` only flashes.

use crate::combat::CombatState;
use crate::combat::hooks::{DamageQuery, Hooks, PowerRef};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct HardToKill;

impl PowerModel for HardToKill {
    fn name(&self) -> &'static str {
        "Hard to Kill"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for HardToKill {
    type Me = PowerRef;

    fn modify_damage_cap(&self, s: &CombatState, me: PowerRef, query: DamageQuery) -> Decimal {
        if query.target == Some(me.owner) {
            Decimal::from(s.power(me.owner, me.id).amount)
        } else {
            Decimal::MAX
        }
    }
}
