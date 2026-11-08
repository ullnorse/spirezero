//! `ForbiddenGrimoirePower` (`Models.Powers/ForbiddenGrimoirePower.cs`): `AfterCombatEnd` adds
//! `Amount` card-removal rewards to the room. That is run-layer state, so the combat engine
//! has nothing to do.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::content::powers::{PowerModel, PowerType};

pub struct ForbiddenGrimoire;

impl PowerModel for ForbiddenGrimoire {
    fn name(&self) -> &'static str {
        "Forbidden Grimoire"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for ForbiddenGrimoire {
    type Me = PowerRef;
}
