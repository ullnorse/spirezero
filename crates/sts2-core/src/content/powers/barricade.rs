//! `BarricadePower` (`Models.Powers/BarricadePower.cs`): the owner's Block is not cleared at the
//! start of its turn. `AfterApplied` only fills the applier's name for the description.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId};
use crate::content::powers::{PowerModel, PowerType};

pub struct Barricade;

impl PowerModel for Barricade {
    fn name(&self) -> &'static str {
        "Barricade"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Barricade {
    type Me = PowerRef;

    fn should_clear_block(&self, _s: &CombatState, me: PowerRef, creature: CreatureId) -> bool {
        me.owner != creature
    }
}
