//! `RunicPyramid` (`Models.Relics/RunicPyramid.cs`): the owner's hand is not flushed at the end
//! of its turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, RelicId};
use crate::content::relics::RelicModel;

pub struct RunicPyramid;

impl RelicModel for RunicPyramid {
    fn name(&self) -> &'static str {
        "Runic Pyramid"
    }
}

#[sts2_macros::hooks]
impl Hooks for RunicPyramid {
    type Me = RelicId;

    fn should_flush(&self, _s: &CombatState, _me: RelicId) -> bool {
        // `player != Owner` is never true: the only player is the owner.
        false
    }
}
