//! `BlackBlood` (`Models.Relics/BlackBlood.cs`): Ironclad's upgraded starter relic
//! (`TouchOfOrobas`); heal 12 after a won combat.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `HealVar(12m)`.
const HEAL: i32 = 12;

pub struct BlackBlood;

impl RelicModel for BlackBlood {
    fn name(&self) -> &'static str {
        "Black Blood"
    }
}

#[sts2_macros::hooks]
impl Hooks for BlackBlood {
    type Me = RelicId;

    fn after_combat_victory(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        let owner = eng.s.player.creature;
        if eng.s.creature(owner).is_dead() {
            return Ok(());
        }
        eng.heal(owner, Decimal::from(HEAL))
    }
}
