//! `NutritiousOyster` (`Models.Relics/NutritiousOyster.cs`): `AfterObtained` gains 11 max HP (and heals as much).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RunCtx, RunError};

/// `MaxHpVar(11m)`.
const MAX_HP: i32 = 11;

pub struct NutritiousOyster;

impl RelicModel for NutritiousOyster {
    fn name(&self) -> &'static str {
        "Nutritious Oyster"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        ctx.state().gain_max_hp(MAX_HP);
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for NutritiousOyster {
    type Me = RelicId;
}
