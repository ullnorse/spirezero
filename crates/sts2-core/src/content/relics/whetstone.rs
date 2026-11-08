//! `Whetstone` (`Models.Relics/Whetstone.cs`): `AfterObtained` upgrades 2 random upgradable Attacks of
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

pub struct Whetstone;

impl RelicModel for Whetstone {
    fn name(&self) -> &'static str {
        "Whetstone"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        let deck = s.deck.clone();
        let mut attacks: Vec<usize> = (0..deck.len())
            .filter(|&i| deck[i].card_type() == Some(CardType::Attack) && deck[i].is_upgradable())
            .collect();
        stable_shuffle_by(&mut attacks, &mut s.rng[RunRngType::Niche], |&a, &b| {
            deck[a].compare_to(&deck[b])
        });
        for &i in attacks.iter().take(CARDS) {
            s.deck[i].upgrade();
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for Whetstone {
    type Me = RelicId;
}
