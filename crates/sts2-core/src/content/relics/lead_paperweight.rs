//! `LeadPaperweight` (`Models.Relics/LeadPaperweight.cs`): `AfterObtained` offers 2 colorless
//! cards (`CreateForReward`, `RegularEncounter` base odds, with the upgrade roll) on a
//! choose-a-card screen that can be skipped; the pick joins the deck.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::odds::CardRarityOddsType;
use crate::run::{CardCreation, CardOdds, CardPool, RunCtx, RunError};

/// `CreateForReward(Owner, 2, options)`.
const CARDS: usize = 2;

pub struct LeadPaperweight;

impl RelicModel for LeadPaperweight {
    fn name(&self) -> &'static str {
        "Lead Paperweight"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        // `new CardCreationOptions([ColorlessCardPool], Source.Other, RegularEncounter)`.
        let options = CardCreation {
            odds: CardOdds::Base(CardRarityOddsType::RegularEncounter),
            upgrade_roll: true,
            ..CardCreation::non_combat_default(CardPool::Colorless)
        };
        let cards = ctx.create_cards(CARDS, options);
        ctx.choose_a_card(cards, None);
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for LeadPaperweight {
    type Me = RelicId;
}
