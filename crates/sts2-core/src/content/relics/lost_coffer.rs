//! `LostCoffer` (`Models.Relics/LostCoffer.cs`): `AfterObtained` offers a card reward (3 cards
//! of the character pool, `RegularEncounter` base odds) and a potion reward
//! (`RewardsCmd.OfferCustom`).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::odds::CardRarityOddsType;
use crate::run::{CardCreation, CardOdds, CardPool, PendingReward, RunCtx, RunError};

/// `new CardReward(options, 3, Owner)`.
const CARDS: usize = 3;

pub struct LostCoffer;

impl RelicModel for LostCoffer {
    fn name(&self) -> &'static str {
        "Lost Coffer"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        // `new CardCreationOptions([Character.CardPool], Source.Other, RegularEncounter)`.
        let options = CardCreation {
            odds: CardOdds::Base(CardRarityOddsType::RegularEncounter),
            upgrade_roll: true,
            ..CardCreation::non_combat_default(CardPool::Character)
        }
        .as_card_reward();
        ctx.offer_pending_rewards(vec![
            PendingReward::Card(options, CARDS),
            PendingReward::Potion,
        ]);
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for LostCoffer {
    type Me = RelicId;
}
