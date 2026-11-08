//! `CallingBell` (`Models.Relics/CallingBell.cs`): `AfterObtained` adds a `CurseOfTheBell` to the
//! deck, then offers 3 relic rewards: a Common, an Uncommon and a Rare one (in test mode
//! `Anchor`, `GremlinHorn` and `MummifiedHand`).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::relic_bag::RelicRarity;
use crate::run::{PendingReward, Reward, RunCard, RunCtx, RunError};

pub struct CallingBell;

impl RelicModel for CallingBell {
    fn name(&self) -> &'static str {
        "Calling Bell"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        s.add_card(RunCard::new("CURSE_OF_THE_BELL"));
        let rewards = if s.test_mode {
            ["ANCHOR", "GREMLIN_HORN", "MUMMIFIED_HAND"]
                .map(|id| PendingReward::Ready(Reward::Relic(id)))
                .to_vec()
        } else {
            [
                RelicRarity::Common,
                RelicRarity::Uncommon,
                RelicRarity::Rare,
            ]
            .map(PendingReward::RelicOf)
            .to_vec()
        };
        ctx.offer_pending_rewards(rewards);
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for CallingBell {
    type Me = RelicId;
}
