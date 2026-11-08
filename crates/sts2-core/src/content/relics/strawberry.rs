//! `Strawberry` (`Models.Relics/Strawberry.cs`): `AfterObtained` gains 7 max HP (and heals as much).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RunCtx, RunError};

/// `MaxHpVar(7m)`.
const MAX_HP: i32 = 7;

pub struct Strawberry;

impl RelicModel for Strawberry {
    fn name(&self) -> &'static str {
        "Strawberry"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        ctx.state().gain_max_hp(MAX_HP);
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for Strawberry {
    type Me = RelicId;
}
