//! `GlassEye` (`Models.Relics/GlassEye.cs`): `AfterObtained` offers 5 card rewards of 3 cards
//! from the character's pool, uniform odds filtered to Common, Common, Uncommon, Uncommon and
//! Rare. `NoRarityModification` has no effect on uniform odds (no rarity is rolled).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::odds::CardRarity;
use crate::run::{CardCreation, CardFilter, CardPool, PendingReward, RunCtx, RunError};

/// The rewards' rarities, in order.
const RARITIES: [CardRarity; 5] = [
    CardRarity::Common,
    CardRarity::Common,
    CardRarity::Uncommon,
    CardRarity::Uncommon,
    CardRarity::Rare,
];
/// `new CardReward(options, 3, Owner)`.
const CARDS: usize = 3;

pub struct GlassEye;

impl RelicModel for GlassEye {
    fn name(&self) -> &'static str {
        "Glass Eye"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let rewards = RARITIES
            .into_iter()
            .map(|rarity| {
                let options = CardCreation::non_combat_uniform(
                    CardPool::Character,
                    CardFilter::Rarity(rarity),
                );
                PendingReward::Card(options, CARDS)
            })
            .collect();
        ctx.offer_pending_rewards(rewards);
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for GlassEye {
    type Me = RelicId;
}
