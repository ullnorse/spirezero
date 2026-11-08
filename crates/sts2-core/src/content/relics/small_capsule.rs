//! `SmallCapsule` (`Models.Relics/SmallCapsule.cs`): `AfterObtained` offers a relic reward
//! (`RewardsCmd.OfferCustom`).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{Reward, RunCtx, RunError};

pub struct SmallCapsule;

impl RelicModel for SmallCapsule {
    fn name(&self) -> &'static str {
        "Small Capsule"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let relic = ctx.pull_relic_from_front();
        ctx.offer_rewards(vec![Reward::Relic(relic)]);
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for SmallCapsule {
    type Me = RelicId;
}
