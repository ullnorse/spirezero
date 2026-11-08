//! `SignetRing` (`Models.Relics/SignetRing.cs`): Ancient (Nonupeipe); `AfterObtained` gains 888
//! gold (`PlayerCmd.GainGold`, so `ModifyGoldGained` applies).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;
use crate::run::{RunCtx, RunError};

/// `GoldVar(888)`.
const GOLD: i32 = 888;

pub struct SignetRing;

impl RelicModel for SignetRing {
    fn name(&self) -> &'static str {
        "Signet Ring"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        ctx.state().gain_gold(Decimal::from(GOLD));
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for SignetRing {
    type Me = RelicId;
}
