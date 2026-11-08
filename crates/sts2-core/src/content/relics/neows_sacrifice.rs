//! `NeowsSacrifice` (`Models.Relics/NeowsSacrifice.cs`): `AfterObtained` procures an
//! `Ambergris` potion (`PotionCmd.TryToProcure`, nothing if the belt is full), then adds a
//! `Guilty` curse to the deck.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RunCard, RunCtx, RunError};

pub struct NeowsSacrifice;

impl RelicModel for NeowsSacrifice {
    fn name(&self) -> &'static str {
        "Neow's Sacrifice"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        ctx.state().procure_potion("AMBERGRIS");
        ctx.state().add_card(RunCard::new("GUILTY"));
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for NeowsSacrifice {
    type Me = RelicId;
}
