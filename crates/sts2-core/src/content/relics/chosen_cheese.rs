//! `ChosenCheese` (`Models.Relics/ChosenCheese.cs`): +1 max HP after every fight.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `MaxHpVar(1m)`.
const MAX_HP: i32 = 1;

pub struct ChosenCheese;

impl RelicModel for ChosenCheese {
    fn name(&self) -> &'static str {
        "Chosen Cheese"
    }
}

#[sts2_macros::hooks]
impl Hooks for ChosenCheese {
    type Me = RelicId;

    fn after_combat_end(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        let owner = eng.s.player.creature;
        eng.gain_max_hp(owner, Decimal::from(MAX_HP))
    }
}
