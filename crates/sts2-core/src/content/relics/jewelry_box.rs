//! `JewelryBox` (`Models.Relics/JewelryBox.cs`): Ancient (Nonupeipe); `AfterObtained` adds an
//! `Apotheosis` to the deck (`CardPileCmd.Add`, so `ModifyCardBeingAddedToDeck` applies).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RunCard, RunCtx, RunError};

pub struct JewelryBox;

impl RelicModel for JewelryBox {
    fn name(&self) -> &'static str {
        "Jewelry Box"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        ctx.state().add_card(RunCard::new("APOTHEOSIS"));
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for JewelryBox {
    type Me = RelicId;
}
