//! `SturdyClamp` (`Models.Relics/SturdyClamp.cs`): the owner's block is not cleared at the start
//! of its turn; all but 10 of it is lost instead.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `_maxRetainedBlock` (also the `BlockVar(10m, ValueProp.Unpowered)` of the description).
const MAX_RETAINED_BLOCK: i32 = 10;

pub struct SturdyClamp;

impl RelicModel for SturdyClamp {
    fn name(&self) -> &'static str {
        "Sturdy Clamp"
    }
}

#[sts2_macros::hooks]
impl Hooks for SturdyClamp {
    type Me = RelicId;

    fn should_clear_block(&self, s: &CombatState, _me: RelicId, creature: CreatureId) -> bool {
        creature != s.player.creature
    }

    fn after_preventing_block_clear(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        creature: CreatureId,
    ) -> Flow {
        // Only the preventer is called, so `this == preventer` holds.
        if creature != eng.s.player.creature {
            return Ok(());
        }
        let block = eng.s.creature(creature).block;
        if block > MAX_RETAINED_BLOCK {
            eng.lose_block(creature, Decimal::from(block - MAX_RETAINED_BLOCK), None)?;
        }
        Ok(())
    }
}
