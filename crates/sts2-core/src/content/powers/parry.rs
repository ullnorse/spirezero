//! `ParryPower` (`Models.Powers/ParryPower.cs`): a buff (`Counter`) with no hooks; each Sovereign
//! Blade its owner plays gains `Amount` block after the attack (`SovereignBlade.OnPlay`).

use crate::combat::hooks::{Hooks, PowerRef};
use crate::content::powers::{PowerModel, PowerType};

pub struct Parry;

impl PowerModel for Parry {
    fn name(&self) -> &'static str {
        "Parry"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Parry {
    type Me = PowerRef;
}
