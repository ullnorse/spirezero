//! `ArcaneScroll` (`Models.Relics/ArcaneScroll.cs`): `AfterObtained` adds 1 uniform rare card of
//! the character pool to the deck (`CreateForReward`, `NoUpgradeRoll`, no choice).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::odds::CardRarity;
use crate::run::{CardCreation, CardFilter, CardPool, RunCtx, RunError};

/// `CardsVar(1)`.
const CARDS: usize = 1;

pub struct ArcaneScroll;

impl RelicModel for ArcaneScroll {
    fn name(&self) -> &'static str {
        "Arcane Scroll"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let options = CardCreation::non_combat_uniform(
            CardPool::Character,
            CardFilter::Rarity(CardRarity::Rare),
        );
        let cards = ctx.create_cards(CARDS, options);
        if let Some(&card) = cards.first() {
            ctx.state().add_card(card);
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for ArcaneScroll {
    type Me = RelicId;
}
