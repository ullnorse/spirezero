//! `Chandelier` (`Models.Relics/Chandelier.cs`): 3 energy at the start of turn 3.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId, Side};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `EnergyVar(3)`.
const ENERGY: i32 = 3;

pub struct Chandelier;

impl RelicModel for Chandelier {
    fn name(&self) -> &'static str {
        "Chandelier"
    }
}

#[sts2_macros::hooks]
impl Hooks for Chandelier {
    type Me = RelicId;

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&eng.s.player.creature) && eng.s.player.turn_number == 3 {
            eng.gain_energy(Decimal::from(ENERGY))?;
        }
        Ok(())
    }
}
