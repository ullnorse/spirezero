//! `Lantern` (`Models.Relics/Lantern.cs`): 1 energy at the start of the first turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId, Side};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `EnergyVar(1)`.
const ENERGY: Decimal = Decimal::ONE;

pub struct Lantern;

impl RelicModel for Lantern {
    fn name(&self) -> &'static str {
        "Lantern"
    }
}

#[sts2_macros::hooks]
impl Hooks for Lantern {
    type Me = RelicId;

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&eng.s.player.creature) && eng.s.player.turn_number <= 1 {
            eng.gain_energy(ENERGY)?;
        }
        Ok(())
    }
}
