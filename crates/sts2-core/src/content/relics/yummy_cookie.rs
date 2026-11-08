//! `YummyCookie` (`Models.Relics/YummyCookie.cs`): Ancient (Tezcatara); `AfterObtained` upgrades 4
//! chosen cards (`FromDeckForUpgrade`). The icon's `Rng.Chaotic` character pick is cosmetic.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RunCtx, RunError, SelectPurpose};

/// `CardsVar(4)`.
const CARDS: usize = 4;

pub struct YummyCookie;

impl RelicModel for YummyCookie {
    fn name(&self) -> &'static str {
        "Yummy Cookie"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let options = ctx.state().upgradable_cards();
        ctx.select(SelectPurpose::Upgrade, options, CARDS)
    }
}

#[sts2_macros::hooks]
impl Hooks for YummyCookie {
    type Me = RelicId;
}
