//! `EmptyCage` (`Models.Relics/EmptyCage.cs`): `AfterObtained` removes 2 chosen cards
//! (`FromDeckForRemoval`, then `RemoveFromDeck` each).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RunCtx, RunError, SelectPurpose};

/// `CardsVar(2)`.
const CARDS: usize = 2;

pub struct EmptyCage;

impl RelicModel for EmptyCage {
    fn name(&self) -> &'static str {
        "Empty Cage"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let options = ctx.state().removable_cards();
        ctx.select(SelectPurpose::Remove { damage: 0 }, options, CARDS)
    }
}

#[sts2_macros::hooks]
impl Hooks for EmptyCage {
    type Me = RelicId;
}
