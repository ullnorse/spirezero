//! `StrengthPower` (`Models.Powers/StrengthPower.cs`): `+Amount` to the owner's powered attacks.

use crate::combat::CombatState;
use crate::combat::hooks::{DamageQuery, Hooks, PowerRef};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Strength;

impl PowerModel for Strength {
    fn name(&self) -> &'static str {
        "Strength"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn allows_negative(&self) -> bool {
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for Strength {
    type Me = PowerRef;

    fn modify_damage_additive(
        &self,
        s: &CombatState,
        me: PowerRef,
        query: DamageQuery,
        _amount: Decimal,
    ) -> Decimal {
        if query.dealer != Some(me.owner) || !query.props.is_powered_attack() {
            return Decimal::ZERO;
        }
        Decimal::from(s.power(me.owner, me.id).amount)
    }
}
