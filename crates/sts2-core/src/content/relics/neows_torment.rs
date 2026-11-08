//! `NeowsTorment` (`Models.Relics/NeowsTorment.cs`): `AfterObtained` adds `NeowsFury` to the
//! deck.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RunCard, RunCtx, RunError};

pub struct NeowsTorment;

impl RelicModel for NeowsTorment {
    fn name(&self) -> &'static str {
        "Neow's Torment"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        ctx.state().add_card(RunCard::new("NEOWS_FURY"));
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for NeowsTorment {
    type Me = RelicId;
}
