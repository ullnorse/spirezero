//! `IvoryTile` (`Models.Relics/IvoryTile.cs`): 1 energy after the owner plays a card that cost at
//! least 3 energy.

use crate::combat::hooks::Hooks;
use crate::combat::{CardPlay, Engine, Flow, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `EnergyVar(1)`.
const ENERGY: Decimal = Decimal::ONE;
/// `EnergyVar("EnergyThreshold", 3)` (`IntValue`).
const ENERGY_THRESHOLD: i32 = 3;

pub struct IvoryTile;

impl RelicModel for IvoryTile {
    fn name(&self) -> &'static str {
        "Ivory Tile"
    }
}

#[sts2_macros::hooks]
impl Hooks for IvoryTile {
    type Me = RelicId;

    fn after_card_played(&self, eng: &mut Engine<'_>, _me: RelicId, play: &CardPlay) -> Flow {
        // The only player owns every card.
        if play.energy_value >= ENERGY_THRESHOLD {
            eng.gain_energy(ENERGY)?;
        }
        Ok(())
    }
}
