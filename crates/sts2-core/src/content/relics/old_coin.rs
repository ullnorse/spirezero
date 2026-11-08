//! `OldCoin` (`Models.Relics/OldCoin.cs`): `AfterObtained` gains 300 gold.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;
use crate::run::RunState;
use crate::run::relic_bag::is_before_act3_treasure_chest;
use crate::run::{RunCtx, RunError};

/// `GoldVar(300)`.
const GOLD: i32 = 300;

pub struct OldCoin;

impl RelicModel for OldCoin {
    /// `IsAllowed` (`OldCoin.cs:20-23`).
    fn is_allowed(&self, s: &RunState) -> bool {
        is_before_act3_treasure_chest(s)
    }

    fn name(&self) -> &'static str {
        "Old Coin"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        ctx.state().gain_gold(Decimal::from(GOLD));
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for OldCoin {
    type Me = RelicId;
}
