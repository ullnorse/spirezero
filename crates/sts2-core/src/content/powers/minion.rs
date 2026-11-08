//! `MinionPower` (`Models.Powers/MinionPower.cs`): marks its owner as a secondary enemy, which
//! dies with the last primary enemy and does not keep combat going. Survives its owner's death
//! and does not trigger Fatal. A buff (`StackType` `Single`).

use crate::combat::CombatState;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::content::powers::{PowerModel, PowerType};

/// Marks a secondary enemy.
pub struct Minion;

impl PowerModel for Minion {
    fn name(&self) -> &'static str {
        "Minion"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn should_be_removed_after_owner_death(&self) -> bool {
        false
    }

    fn should_owner_death_trigger_fatal(&self, _s: &CombatState, _me: PowerRef) -> bool {
        false
    }

    fn owner_is_secondary_enemy(&self) -> bool {
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for Minion {
    type Me = PowerRef;
}
