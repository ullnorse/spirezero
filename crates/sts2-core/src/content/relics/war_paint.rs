//! `WarPaint` (`Models.Relics/WarPaint.cs`): `AfterObtained` upgrades 2 random upgradable Skills of
//! the deck (`StableShuffle` on `Rng.Niche`).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::cards::CardType;
use crate::content::relics::RelicModel;
use crate::rng::RunRngType;
use crate::run::{RunCtx, RunError};
use crate::shuffle::stable_shuffle_by;

/// `CardsVar(2)`.
const CARDS: usize = 2;

pub struct WarPaint;

impl RelicModel for WarPaint {
    fn name(&self) -> &'static str {
        "War Paint"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        let deck = s.deck.clone();
        let mut skills: Vec<usize> = (0..deck.len())
            .filter(|&i| deck[i].card_type() == Some(CardType::Skill) && deck[i].is_upgradable())
            .collect();
        stable_shuffle_by(&mut skills, &mut s.rng[RunRngType::Niche], |&a, &b| {
            deck[a].compare_to(&deck[b])
        });
        for &i in skills.iter().take(CARDS) {
            s.deck[i].upgrade();
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for WarPaint {
    type Me = RelicId;
}
