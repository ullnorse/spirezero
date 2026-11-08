//! `RunicCapacitor` (`Models.Relics/RunicCapacitor.cs`): 3 more orb slots on the owner's first
//! turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId, Side};
use crate::content::relics::RelicModel;

/// `RepeatVar(3)`.
const REPEAT: i32 = 3;

pub struct RunicCapacitor;

impl RelicModel for RunicCapacitor {
    fn name(&self) -> &'static str {
        "Runic Capacitor"
    }
}

#[sts2_macros::hooks]
impl Hooks for RunicCapacitor {
    type Me = RelicId;

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&eng.s.player.creature) && eng.s.player.turn_number <= 1 {
            eng.add_orb_slots(REPEAT);
        }
        Ok(())
    }
}
