//! `PhylacteryUnbound` (`Models.Relics/PhylacteryUnbound.cs`): Necrobinder's upgraded starter
//! relic (`TouchOfOrobas`); Summon 5 before combat and 2 at the start of each of the owner's
//! turns (`AfterSideTurnStart`, every turn including the first).

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId, Side};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `SummonVar("StartOfCombat", 5m)`.
const START_OF_COMBAT: Decimal = Decimal::new(5, 0);
/// `SummonVar("StartOfTurn", 2m)`.
const START_OF_TURN: Decimal = Decimal::new(2, 0);

pub struct PhylacteryUnbound;

impl RelicModel for PhylacteryUnbound {
    fn name(&self) -> &'static str {
        "Phylactery Unbound"
    }
}

#[sts2_macros::hooks]
impl Hooks for PhylacteryUnbound {
    type Me = RelicId;

    fn before_combat_start(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        eng.summon_osty(START_OF_COMBAT)
    }

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&eng.s.player.creature) {
            eng.summon_osty(START_OF_TURN)?;
        }
        Ok(())
    }
}
