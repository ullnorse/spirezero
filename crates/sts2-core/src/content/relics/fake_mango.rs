//! `FakeMango` (`Models.Relics/FakeMango.cs`): `FakeMerchant`'s `Mango`; `AfterObtained` gains 3
//! max HP (and heals as much).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RunCtx, RunError};

/// `MaxHpVar(3m)`.
const MAX_HP: i32 = 3;

pub struct FakeMango;

impl RelicModel for FakeMango {
    fn name(&self) -> &'static str {
        "Mango???"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        ctx.state().gain_max_hp(MAX_HP);
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for FakeMango {
    type Me = RelicId;
}
