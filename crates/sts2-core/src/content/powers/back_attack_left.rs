//! `BackAttackLeftPower` (`Models.Powers/BackAttackLeftPower.cs`): a marker buff (`Single`) that
//! `SurroundedPower` checks for; no behaviour of its own. The `KaiserCrabBoss`'s Crusher holds it.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::content::powers::{PowerModel, PowerType};

/// Marks the left crab arm.
pub struct BackAttackLeft;

impl PowerModel for BackAttackLeft {
    fn name(&self) -> &'static str {
        "Back Attack"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for BackAttackLeft {
    type Me = PowerRef;
}
