//! `Orrery` (`Models.Relics/Orrery.cs`): `AfterObtained` offers 5 card rewards of 3 cards each from
//! the character pool, `RegularEncounter` base odds (`RewardsCmd.OfferCustom`).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::odds::CardRarityOddsType;
use crate::run::{CardCreation, CardOdds, CardPool, PendingReward, RunCtx, RunError};

/// `CardsVar(5)`.
const REWARDS: usize = 5;
/// `new CardReward(options, 3, Owner)`.
const CARDS: usize = 3;

pub struct Orrery;

impl RelicModel for Orrery {
    fn name(&self) -> &'static str {
        "Orrery"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        // `new CardCreationOptions([Character.CardPool], Source.Other, RegularEncounter)`.
        let options = CardCreation {
            odds: CardOdds::Base(CardRarityOddsType::RegularEncounter),
            upgrade_roll: true,
            ..CardCreation::non_combat_default(CardPool::Character)
        }
        .as_card_reward();
        ctx.offer_pending_rewards(vec![PendingReward::Card(options, CARDS); REWARDS]);
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for Orrery {
    type Me = RelicId;
}
