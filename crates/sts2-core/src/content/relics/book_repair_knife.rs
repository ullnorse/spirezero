//! `BookRepairKnife` (`Models.Relics/BookRepairKnife.cs`): heals 3 for each other creature that
//! died to Doom (and whose powers all allow the Fatal trigger).

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `HealVar(3m)`.
const HEAL: i32 = 3;

pub struct BookRepairKnife;

impl RelicModel for BookRepairKnife {
    fn name(&self) -> &'static str {
        "Book Repair Knife"
    }
}

#[sts2_macros::hooks]
impl Hooks for BookRepairKnife {
    type Me = RelicId;

    fn after_died_to_doom(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        creatures: &[CreatureId],
    ) -> Flow {
        let owner = eng.s.player.creature;
        let count = creatures
            .iter()
            .filter(|&&c| c != owner && eng.target_allows_fatal(c))
            .count();
        if count == 0 {
            return Ok(());
        }
        let count = i32::try_from(count).expect("creature count fits i32");
        eng.heal(owner, Decimal::from(HEAL) * Decimal::from(count))
    }
}
