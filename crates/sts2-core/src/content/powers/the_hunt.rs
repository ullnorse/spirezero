//! `TheHuntPower` (`Models.Powers/TheHuntPower.cs`): a buff (`Counter`) with no hooks; only a
//! visual marker that `TheHunt` was successful.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::content::powers::{PowerModel, PowerType};

pub struct TheHunt;

impl PowerModel for TheHunt {
    fn name(&self) -> &'static str {
        "The Hunt"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for TheHunt {
    type Me = PowerRef;
}
