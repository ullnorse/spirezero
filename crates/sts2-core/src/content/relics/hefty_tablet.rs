//! `HeftyTablet` (`Models.Relics/HeftyTablet.cs`): `AfterObtained` offers 3 uniform rare cards
//! of the character pool (`CreateForReward`, `NoUpgradeRoll`) on a choose-a-card screen that can
//! be skipped; the pick, then an `Injury` curse, join the deck.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::odds::CardRarity;
use crate::run::{CardCreation, CardFilter, CardPool, RunCtx, RunError};

/// `CardsVar(3)`.
const CARDS: usize = 3;

pub struct HeftyTablet;

impl RelicModel for HeftyTablet {
    fn name(&self) -> &'static str {
        "Hefty Tablet"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let options = CardCreation::non_combat_uniform(
            CardPool::Character,
            CardFilter::Rarity(CardRarity::Rare),
        );
        let cards = ctx.create_cards(CARDS, options);
        ctx.choose_a_card(cards, Some("INJURY"));
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for HeftyTablet {
    type Me = RelicId;
}
