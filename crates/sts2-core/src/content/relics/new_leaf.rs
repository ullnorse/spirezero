//! `NewLeaf` (`Models.Relics/NewLeaf.cs`): `AfterObtained` transforms 1 chosen card
//! (`FromDeckForTransformation`, then `CardCmd.TransformToRandom` on the run's `Niche` stream).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RunCtx, RunError, SelectPurpose};

/// `CardsVar(1)`.
const CARDS: usize = 1;

pub struct NewLeaf;

impl RelicModel for NewLeaf {
    fn name(&self) -> &'static str {
        "New Leaf"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let options = ctx.state().transformable_cards();
        ctx.select(SelectPurpose::TransformRandom, options, CARDS)
    }
}

#[sts2_macros::hooks]
impl Hooks for NewLeaf {
    type Me = RelicId;
}
