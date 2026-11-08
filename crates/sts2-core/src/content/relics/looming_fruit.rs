//! `LoomingFruit` (`Models.Relics/LoomingFruit.cs`): Ancient (Nonupeipe, shared pool);
//! `AfterObtained` gains 31 max HP (and heals as much). The icon (`HasCornucopia`) is cosmetic.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RunCtx, RunError};

/// `MaxHpVar(31m)`.
const MAX_HP: i32 = 31;

pub struct LoomingFruit;

impl RelicModel for LoomingFruit {
    fn name(&self) -> &'static str {
        "Looming Fruit"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        ctx.state().gain_max_hp(MAX_HP);
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for LoomingFruit {
    type Me = RelicId;
}
