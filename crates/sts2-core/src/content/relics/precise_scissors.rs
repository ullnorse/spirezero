//! `PreciseScissors` (`Models.Relics/PreciseScissors.cs`): `AfterObtained` removes 1 chosen card
//! (`FromDeckForRemoval`).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RunCtx, RunError, SelectPurpose};

/// `CardsVar(1)`.
const CARDS: usize = 1;

pub struct PreciseScissors;

impl RelicModel for PreciseScissors {
    fn name(&self) -> &'static str {
        "Precise Scissors"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let options = ctx.state().removable_cards();
        ctx.select(SelectPurpose::Remove { damage: 0 }, options, CARDS)
    }
}

#[sts2_macros::hooks]
impl Hooks for PreciseScissors {
    type Me = RelicId;
}
