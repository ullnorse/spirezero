//! `Mango` (`Models.Relics/Mango.cs`): `AfterObtained` gains 14 max HP (and heals as much).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RunCtx, RunError};

/// `MaxHpVar(14m)`.
const MAX_HP: i32 = 14;

pub struct Mango;

impl RelicModel for Mango {
    fn name(&self) -> &'static str {
        "Mango"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        ctx.state().gain_max_hp(MAX_HP);
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for Mango {
    type Me = RelicId;
}
