//! `FanOfKnivesPower` (`Models.Powers/FanOfKnivesPower.cs`): a buff (`Single`) with no hooks;
//! its owner's Shivs target every enemy (`Shiv.TargetType`).

use crate::combat::hooks::{Hooks, PowerRef};
use crate::content::powers::{PowerModel, PowerType};

pub struct FanOfKnives;

impl PowerModel for FanOfKnives {
    fn name(&self) -> &'static str {
        "Fan of Knives"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for FanOfKnives {
    type Me = PowerRef;
}
