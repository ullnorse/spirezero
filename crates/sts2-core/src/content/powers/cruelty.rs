//! `CrueltyPower` (`Models.Powers/CrueltyPower.cs`): the owner's powered attacks make Vulnerable
//! `Amount` percent stronger. It has no hooks; `VulnerablePower` asks it
//! (`ModifyVulnerableMultiplier`) for the dealer's Cruelty, or its pet owner's. Only the Ironclad
//! `Cruelty` card applies it.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, ValueProps};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Cruelty;

impl PowerModel for Cruelty {
    fn name(&self) -> &'static str {
        "Cruelty"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Cruelty {
    type Me = PowerRef;
}

/// `CrueltyPower.ModifyVulnerableMultiplier(target, amount, props, dealer, cardSource)` for a
/// Cruelty of `percent` owned by `owner`.
pub fn modify_vulnerable_multiplier(
    owner: CreatureId,
    percent: i32,
    target: CreatureId,
    amount: Decimal,
    props: ValueProps,
) -> Decimal {
    if target == owner || !props.is_powered_attack() {
        return amount;
    }
    // `(decimal)Amount / 100m`.
    amount + Decimal::new(i128::from(percent), 2)
}
