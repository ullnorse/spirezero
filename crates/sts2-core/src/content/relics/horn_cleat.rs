//! `HornCleat` (`Models.Relics/HornCleat.cs`): 14 Block when the owner's Block is cleared at the
//! start of turn 2.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId, ValueProps};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `BlockVar(14m, ValueProp.Unpowered)`.
const BLOCK: i32 = 14;

pub struct HornCleat;

impl RelicModel for HornCleat {
    fn name(&self) -> &'static str {
        "Horn Cleat"
    }
}

#[sts2_macros::hooks]
impl Hooks for HornCleat {
    type Me = RelicId;

    fn after_block_cleared(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        creature: CreatureId,
    ) -> Flow {
        let owner = eng.s.player.creature;
        if creature == owner && eng.s.player.turn_number == 2 {
            eng.gain_block(owner, Decimal::from(BLOCK), ValueProps::UNPOWERED, None)?;
        }
        Ok(())
    }
}
