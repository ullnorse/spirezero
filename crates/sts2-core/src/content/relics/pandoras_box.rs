//! `PandorasBox` (`Models.Relics/PandorasBox.cs`): `AfterObtained` transforms every removable
//! Basic Strike and Defend in the deck at once (`CardCmd.Transform` of the list, each
//! replacement drawn by `CreateRandomCardForTransform` on the run's `Niche` stream, in deck
//! order). The result screen is UI only.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::rng::RunRngType;
use crate::run::{RunCtx, RunError};

pub struct PandorasBox;

impl RelicModel for PandorasBox {
    fn name(&self) -> &'static str {
        "Pandora's Box"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        let cards: Vec<usize> = (0..s.deck.len())
            .filter(|&i| {
                let c = s.deck[i];
                (c.is_strike() || c.is_defend()) && c.is_removable()
            })
            .collect();
        let mut rng = s.rng[RunRngType::Niche].clone();
        let result = s.transform_many_to_random(&cards, &mut rng);
        s.rng[RunRngType::Niche] = rng;
        result
    }
}

#[sts2_macros::hooks]
impl Hooks for PandorasBox {
    type Me = RelicId;
}
