//! `GoldenPearl` (`Models.Relics/GoldenPearl.cs`): `AfterObtained` gains 150 gold.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;
use crate::run::{RunCtx, RunError};

/// `GoldVar(150)`.
const GOLD: i32 = 150;

pub struct GoldenPearl;

impl RelicModel for GoldenPearl {
    fn name(&self) -> &'static str {
        "Golden Pearl"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        ctx.state().gain_gold(Decimal::from(GOLD));
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for GoldenPearl {
    type Me = RelicId;
}
