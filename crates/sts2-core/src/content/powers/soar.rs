//! `SoarPower` (`Models.Powers/SoarPower.cs`): a buff (`Single`) on `OwlMagistrate` halving the
//! powered attack damage it takes (`DamageDecrease` 50).

use crate::combat::CombatState;
use crate::combat::hooks::{DamageQuery, Hooks, PowerRef};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Soar;

/// `DamageDecrease` (50) / 100.
const DAMAGE_DECREASE: Decimal = Decimal::new(5, 1);

impl PowerModel for Soar {
    fn name(&self) -> &'static str {
        "Soar"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Soar {
    type Me = PowerRef;

    fn modify_damage_multiplicative(
        &self,
        _s: &CombatState,
        me: PowerRef,
        query: DamageQuery,
        _amount: Decimal,
    ) -> Decimal {
        if query.target != Some(me.owner) || !query.props.is_powered_attack() {
            return Decimal::ONE;
        }
        DAMAGE_DECREASE
    }
}
