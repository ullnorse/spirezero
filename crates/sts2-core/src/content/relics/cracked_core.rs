//! `CrackedCore` (`Models.Relics/CrackedCore.cs`): Defect's starter relic; channels 1 Lightning
//! orb before the owner's first turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, OrbKind, RelicId, Side};
use crate::content::relics::RelicModel;

/// `DynamicVar("Lightning", 1m)`.
const LIGHTNING: u32 = 1;

pub struct CrackedCore;

impl RelicModel for CrackedCore {
    fn name(&self) -> &'static str {
        "Cracked Core"
    }
}

#[sts2_macros::hooks]
impl Hooks for CrackedCore {
    type Me = RelicId;

    fn before_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        let owner = eng.s.player.creature;
        if participants.contains(&owner) && eng.s.player.turn_number <= 1 {
            for _ in 0..LIGHTNING {
                eng.channel(OrbKind::Lightning)?;
            }
        }
        Ok(())
    }
}
