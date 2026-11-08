//! `BackAttackRightPower` (`Models.Powers/BackAttackRightPower.cs`): a marker buff (`Single`) that
//! `SurroundedPower` checks for; no behaviour of its own. The `KaiserCrabBoss`'s Rocket holds it.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::content::powers::{PowerModel, PowerType};

/// Marks the right crab arm.
pub struct BackAttackRight;

impl PowerModel for BackAttackRight {
    fn name(&self) -> &'static str {
        "Back Attack"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for BackAttackRight {
    type Me = PowerRef;
}
