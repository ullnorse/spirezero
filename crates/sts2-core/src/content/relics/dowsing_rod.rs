//! `DowsingRod` (`Models.Relics/DowsingRod.cs`): `AfterObtained` adds a `Dowsing` quest card to
//! the deck.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RunCard, RunCtx, RunError};

pub struct DowsingRod;

impl RelicModel for DowsingRod {
    fn name(&self) -> &'static str {
        "Dowsing Rod"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        ctx.state().add_card(RunCard::new("DOWSING"));
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for DowsingRod {
    type Me = RelicId;
}
