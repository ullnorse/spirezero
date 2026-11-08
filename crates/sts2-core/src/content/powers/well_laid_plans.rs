//! `WellLaidPlansPower` (`Models.Powers/WellLaidPlansPower.cs`): a buff (`Single`); the owner's
//! hand is not flushed at the end of its turn.

use crate::combat::CombatState;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::content::powers::{PowerModel, PowerType};

pub struct WellLaidPlans;

impl PowerModel for WellLaidPlans {
    fn name(&self) -> &'static str {
        "Well-Laid Plans"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for WellLaidPlans {
    type Me = PowerRef;

    // The owner is always the player (`player != Owner.Player` is never true).
    fn should_flush(&self, _s: &CombatState, _me: PowerRef) -> bool {
        false
    }
}
