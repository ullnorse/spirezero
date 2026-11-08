//! Test-only relic whose victory hook changes the run, then meets content that is not ported
//! (as `SwordOfStone` would replacing itself with an unported relic).

#![cfg(test)]

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RoomType, RunCtx, RunError, RunState};

/// Test-only: a failing `AfterCombatVictory`.
pub struct TestUnportedTrophy;

impl RelicModel for TestUnportedTrophy {
    fn name(&self) -> &'static str {
        "Test Unported Trophy"
    }

    fn victory_in_run_may_fail(&self, _s: &RunState, _me: usize) -> bool {
        true
    }

    fn after_combat_victory_in_run(
        &self,
        ctx: &mut RunCtx<'_>,
        me: usize,
        _room: RoomType,
    ) -> Result<(), RunError> {
        ctx.state().gold += 100;
        ctx.replace_relic(me, "NOT_A_PORTED_RELIC")
    }
}

#[sts2_macros::hooks]
impl Hooks for TestUnportedTrophy {
    type Me = RelicId;
}
