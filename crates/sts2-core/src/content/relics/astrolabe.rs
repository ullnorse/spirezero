//! `Astrolabe` (`Models.Relics/Astrolabe.cs`): `AfterObtained` transforms 3 chosen cards
//! (`FromDeckForTransformation`), each into a random card (`CreateRandomCardForTransform` on the
//! run's `Niche` stream) that is upgraded before `CardCmd.Transform`, one card at a time.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RunCtx, RunError, SelectPurpose};

/// `CardsVar(3)`.
const CARDS: usize = 3;

pub struct Astrolabe;

impl RelicModel for Astrolabe {
    fn name(&self) -> &'static str {
        "Astrolabe"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let options = ctx.state().transformable_cards();
        ctx.select(SelectPurpose::TransformRandomUpgraded, options, CARDS)
    }
}

#[sts2_macros::hooks]
impl Hooks for Astrolabe {
    type Me = RelicId;
}
