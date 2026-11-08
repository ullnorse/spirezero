//! `GremlinHorn` (`Models.Relics/GremlinHorn.cs`): 1 energy and 1 card whenever an enemy dies.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `EnergyVar(1)`.
const ENERGY: Decimal = Decimal::ONE;
/// `CardsVar(1)`.
const CARDS: Decimal = Decimal::ONE;

pub struct GremlinHorn;

impl RelicModel for GremlinHorn {
    fn name(&self) -> &'static str {
        "Gremlin Horn"
    }
}

#[sts2_macros::hooks]
impl Hooks for GremlinHorn {
    type Me = RelicId;

    fn after_death(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        creature: CreatureId,
        _was_removal_prevented: bool,
    ) -> Flow {
        let owner = eng.s.player.creature;
        if eng.s.creature(creature).side != eng.s.creature(owner).side {
            eng.gain_energy(ENERGY)?;
            eng.draw(CARDS, false)?;
        }
        Ok(())
    }
}
