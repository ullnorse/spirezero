//! `GoldPlatedCables` (`Models.Relics/GoldPlatedCables.cs`): the owner's first orb in the queue
//! triggers its passive one more time. `AfterModifyingOrbPassiveTriggerCount` only flashes.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, Orb, RelicId};
use crate::content::relics::RelicModel;

pub struct GoldPlatedCables;

impl RelicModel for GoldPlatedCables {
    fn name(&self) -> &'static str {
        "Gold-Plated Cables"
    }
}

#[sts2_macros::hooks]
impl Hooks for GoldPlatedCables {
    type Me = RelicId;

    fn modify_orb_passive_trigger_counts(
        &self,
        s: &CombatState,
        _me: RelicId,
        orb: Orb,
        count: i32,
    ) -> i32 {
        // `orb.Owner != Owner`: every orb belongs to the only player.
        if s.player.orbs.first().map(|o| o.id) == Some(orb.id) {
            count + 1
        } else {
            count
        }
    }
}
