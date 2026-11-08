//! `PaelsHorn` (`Models.Relics/PaelsHorn.cs`): Ancient (Pael); `AfterObtained` adds 2 `Relax` to
//! the deck.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RunCard, RunCtx, RunError};

/// The loop count in `AfterObtained`.
const CARDS: usize = 2;

pub struct PaelsHorn;

impl RelicModel for PaelsHorn {
    fn name(&self) -> &'static str {
        "Pael's Horn"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        for _ in 0..CARDS {
            ctx.state().add_card(RunCard::new("RELAX"));
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for PaelsHorn {
    type Me = RelicId;
}
