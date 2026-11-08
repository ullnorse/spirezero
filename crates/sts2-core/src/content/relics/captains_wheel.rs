//! `CaptainsWheel` (`Models.Relics/CaptainsWheel.cs`): 18 Block when the owner's block is cleared
//! at the start of turn 3.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId, ValueProps};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `BlockVar(18m, ValueProp.Unpowered)`.
const BLOCK: i32 = 18;

pub struct CaptainsWheel;

impl RelicModel for CaptainsWheel {
    fn name(&self) -> &'static str {
        "Captain's Wheel"
    }
}

#[sts2_macros::hooks]
impl Hooks for CaptainsWheel {
    type Me = RelicId;

    fn after_block_cleared(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        creature: CreatureId,
    ) -> Flow {
        let owner = eng.s.player.creature;
        if creature == owner && eng.s.player.turn_number == 3 {
            eng.gain_block(owner, Decimal::from(BLOCK), ValueProps::UNPOWERED, None)?;
        }
        Ok(())
    }
}
