//! `Storybook` (`Models.Relics/Storybook.cs`): Ancient (Tezcatara); `AfterObtained` adds a
//! `BrightestFlame` to the deck.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RunCard, RunCtx, RunError};

pub struct Storybook;

impl RelicModel for Storybook {
    fn name(&self) -> &'static str {
        "Storybook"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        ctx.state().add_card(RunCard::new("BRIGHTEST_FLAME"));
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for Storybook {
    type Me = RelicId;
}
