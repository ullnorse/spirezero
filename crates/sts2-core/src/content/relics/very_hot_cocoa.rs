//! `VeryHotCocoa` (`Models.Relics/VeryHotCocoa.cs`): Ancient (Tezcatara); 4 energy at the start
//! of the owner's first turn (`TurnNumber <= 1`).

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId, Side};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `EnergyVar(4)`.
const ENERGY: Decimal = Decimal::new(4, 0);

pub struct VeryHotCocoa;

impl RelicModel for VeryHotCocoa {
    fn name(&self) -> &'static str {
        "Very Hot Cocoa"
    }
}

#[sts2_macros::hooks]
impl Hooks for VeryHotCocoa {
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
