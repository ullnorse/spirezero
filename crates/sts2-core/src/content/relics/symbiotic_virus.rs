//! `SymbioticVirus` (`Models.Relics/SymbioticVirus.cs`): channels 1 Dark orb on the owner's first
//! turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, OrbKind, RelicId, Side};
use crate::content::relics::RelicModel;

/// `DynamicVar("Dark", 1m)`.
const DARK: u32 = 1;

pub struct SymbioticVirus;

impl RelicModel for SymbioticVirus {
    fn name(&self) -> &'static str {
        "Symbiotic Virus"
    }
}

#[sts2_macros::hooks]
impl Hooks for SymbioticVirus {
    type Me = RelicId;

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&eng.s.player.creature) && eng.s.player.turn_number <= 1 {
            for _ in 0..DARK {
                eng.channel(OrbKind::Dark)?;
            }
        }
        Ok(())
    }
}
