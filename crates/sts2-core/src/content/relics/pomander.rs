//! `Pomander` (`Models.Relics/Pomander.cs`): `AfterObtained` upgrades 1 chosen card
//! (`FromDeckForUpgrade`).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RunCtx, RunError, SelectPurpose};

/// `CardsVar(1)`.
const CARDS: usize = 1;

pub struct Pomander;

impl RelicModel for Pomander {
    fn name(&self) -> &'static str {
        "Pomander"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let options = ctx.state().upgradable_cards();
        ctx.select(SelectPurpose::Upgrade, options, CARDS)
    }
}

#[sts2_macros::hooks]
impl Hooks for Pomander {
    type Me = RelicId;
}
