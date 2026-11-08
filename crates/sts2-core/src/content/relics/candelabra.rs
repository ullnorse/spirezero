//! `Candelabra` (`Models.Relics/Candelabra.cs`): 2 energy at the start of turn 2.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId, Side};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `EnergyVar(2)`.
const ENERGY: i32 = 2;

pub struct Candelabra;

impl RelicModel for Candelabra {
    fn name(&self) -> &'static str {
        "Candelabra"
    }
}

#[sts2_macros::hooks]
impl Hooks for Candelabra {
    type Me = RelicId;

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&eng.s.player.creature) && eng.s.player.turn_number == 2 {
            eng.gain_energy(Decimal::from(ENERGY))?;
        }
        Ok(())
    }
}
