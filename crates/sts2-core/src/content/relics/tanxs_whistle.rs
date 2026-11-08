//! `TanxsWhistle` (`Models.Relics/TanxsWhistle.cs`): Ancient (Tanx); `AfterObtained` adds a
//! `Whistle` to the deck.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RunCard, RunCtx, RunError};

pub struct TanxsWhistle;

impl RelicModel for TanxsWhistle {
    fn name(&self) -> &'static str {
        "Tanx's Whistle"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        ctx.state().add_card(RunCard::new("WHISTLE"));
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for TanxsWhistle {
    type Me = RelicId;
}
