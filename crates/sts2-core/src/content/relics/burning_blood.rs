//! `BurningBlood` (`Models.Relics/BurningBlood.cs`): Ironclad's starter relic; heal 6 after a won
//! combat.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `HealVar(6m)`.
const HEAL: i32 = 6;

pub struct BurningBlood;

impl RelicModel for BurningBlood {
    fn name(&self) -> &'static str {
        "Burning Blood"
    }
}

#[sts2_macros::hooks]
impl Hooks for BurningBlood {
    type Me = RelicId;

    fn after_combat_victory(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        let owner = eng.s.player.creature;
        if eng.s.creature(owner).is_dead() {
            return Ok(());
        }
        eng.heal(owner, Decimal::from(HEAL))
    }
}
