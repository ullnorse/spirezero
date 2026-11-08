//! `AccelerantPower` (`Models.Powers/AccelerantPower.cs`): does nothing itself; `PoisonPower`
//! reads its amount to trigger extra times.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::content::powers::{PowerModel, PowerType};

pub struct Accelerant;

impl PowerModel for Accelerant {
    fn name(&self) -> &'static str {
        "Accelerant"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Accelerant {
    type Me = PowerRef;
}
