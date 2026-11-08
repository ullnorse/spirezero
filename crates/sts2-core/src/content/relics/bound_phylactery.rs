//! `BoundPhylactery` (`Models.Relics/BoundPhylactery.cs`): Summon 1 before combat and at the
//! start of every later turn (after the energy reset, before the hand draw).

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `SummonVar(1)`.
const SUMMON: Decimal = Decimal::ONE;

pub struct BoundPhylactery;

impl RelicModel for BoundPhylactery {
    fn name(&self) -> &'static str {
        "Bound Phylactery"
    }
}

#[sts2_macros::hooks]
impl Hooks for BoundPhylactery {
    type Me = RelicId;

    fn before_combat_start(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        eng.summon_osty(SUMMON)
    }

    fn after_energy_reset_late(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        // The only player is the owner.
        if eng.s.player.turn_number != 1 {
            eng.summon_osty(SUMMON)?;
        }
        Ok(())
    }
}
