//! `SandCastle` (`Models.Relics/SandCastle.cs`): `AfterObtained` upgrades 6 random upgradable
//! cards (the upgradable deck cards in deck order, `StableShuffle` on the run's `Niche` stream,
//! then the first 6).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::rng::RunRngType;
use crate::run::{RunCtx, RunError};
use crate::shuffle::stable_shuffle_by;

/// `CardsVar(6)`.
const CARDS: usize = 6;

pub struct SandCastle;

impl RelicModel for SandCastle {
    fn name(&self) -> &'static str {
        "Sand Castle"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        let mut cards = s.upgradable_cards();
        let deck = &s.deck;
        stable_shuffle_by(&mut cards, &mut s.rng[RunRngType::Niche], |&a, &b| {
            deck[a].compare_to(&deck[b])
        });
        for i in cards.into_iter().take(CARDS) {
            s.deck[i].upgrade();
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for SandCastle {
    type Me = RelicId;
}
