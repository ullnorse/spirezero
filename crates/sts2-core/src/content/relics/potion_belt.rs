//! `PotionBelt` (`Models.Relics/PotionBelt.cs`): `AfterObtained` adds 2 potion slots.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RunCtx, RunError};

/// `DynamicVar("PotionSlots", 2m)`.
const POTION_SLOTS: usize = 2;

pub struct PotionBelt;

impl RelicModel for PotionBelt {
    fn name(&self) -> &'static str {
        "Potion Belt"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        ctx.state().gain_max_potion_count(POTION_SLOTS);
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for PotionBelt {
    type Me = RelicId;
}
