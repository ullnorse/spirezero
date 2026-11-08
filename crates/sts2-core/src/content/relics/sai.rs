//! `Sai` (`Models.Relics/Sai.cs`): Ancient (Tanx); 7 unpowered Block at the start of each of the
//! owner's turns.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId, Side, ValueProps};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `BlockVar(7m, ValueProp.Unpowered)`.
const BLOCK: i32 = 7;

pub struct Sai;

impl RelicModel for Sai {
    fn name(&self) -> &'static str {
        "Sai"
    }
}

#[sts2_macros::hooks]
impl Hooks for Sai {
    type Me = RelicId;

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        let owner = eng.s.player.creature;
        if participants.contains(&owner) {
            eng.gain_block(owner, Decimal::from(BLOCK), ValueProps::UNPOWERED, None)?;
        }
        Ok(())
    }
}
