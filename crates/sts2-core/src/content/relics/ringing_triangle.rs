//! `RingingTriangle` (`Models.Relics/RingingTriangle.cs`): the owner's hand is not flushed at the
//! end of the first turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, RelicId};
use crate::content::relics::RelicModel;

pub struct RingingTriangle;

impl RelicModel for RingingTriangle {
    fn name(&self) -> &'static str {
        "Ringing Triangle"
    }
}

#[sts2_macros::hooks]
impl Hooks for RingingTriangle {
    type Me = RelicId;

    fn should_flush(&self, s: &CombatState, _me: RelicId) -> bool {
        // `player != Owner` flushes; the only player is the owner.
        s.player.turn_number > 1
    }
}
