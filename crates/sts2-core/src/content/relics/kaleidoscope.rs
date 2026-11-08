//! `Kaleidoscope` (`Models.Relics/Kaleidoscope.cs`): `AfterObtained` offers 2 card rewards, each
//! one card from 3 of the other characters' pools (the pools `StableShuffle`d on `Rng.Niche`),
//! through `RewardsCmd.OfferCustom`.
//!
//! The rewards' reroll options have no card pool, so the engine refuses a `Driftwood` reroll of
//! them.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::characters::Character;
use crate::content::relics::RelicModel;
use crate::rng::RunRngType;
use crate::run::odds::CardRarityOddsType;
use crate::run::{
    CardCreation, CardFilter, CardOdds, CardPool, CardReward, Reward, RunCtx, RunError, hooks,
};
use crate::shuffle::stable_shuffle_by;

/// `CardsVar(2)`: the rewards offered.
const REWARDS: usize = 2;
/// The pools each reward takes a card from (`Take(3)`).
const POOLS: usize = 3;

pub struct Kaleidoscope;

impl RelicModel for Kaleidoscope {
    fn name(&self) -> &'static str {
        "Kaleidoscope"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let own = ctx.state().character;
        let mut rewards: Vec<Vec<_>> = Vec::with_capacity(REWARDS);
        for _ in 0..REWARDS {
            // The other pools, `StableShuffle`d (sorted by the pool's id, i.e. the class name).
            let mut pools: Vec<Character> = Character::MODEL_DB_ORDER
                .into_iter()
                .filter(|&c| c != own)
                .collect();
            stable_shuffle_by(
                &mut pools,
                &mut ctx.state().rng[RunRngType::Niche],
                |a, b| a.name().cmp(b.name()),
            );
            let cards = pools
                .into_iter()
                .take(POOLS)
                .map(|pool| {
                    // `new CardCreationOptions([pool], Other, RegularEncounter)` with
                    // `NoCardPoolModifications`; a card's creation runs the reward-option hooks.
                    let options = CardCreation {
                        odds: CardOdds::Base(CardRarityOddsType::RegularEncounter),
                        upgrade_roll: true,
                        ..CardCreation::non_combat_default(CardPool::Of(pool))
                    }
                    .without_pool_modifications();
                    ctx.create_cards(1, options)[0]
                })
                .collect();
            rewards.push(cards);
        }
        // `OfferCustom` populates each manually-set `CardReward`, which runs the reward-option
        // hooks once more (`CardReward.cs:131-140,146-154`: `Uniform` odds, `NoCardPoolModifications`,
        // `IsCardReward`).
        let populate = CardCreation::non_combat_uniform(CardPool::Character, CardFilter::Any)
            .without_pool_modifications()
            .as_card_reward();
        let s = ctx.state();
        for cards in &mut rewards {
            hooks::modify_card_reward_options(s, cards, populate);
        }
        // `rerollOptions` is `ForNonCombatWithDefaultOdds([])`: no card pool.
        let rewards = rewards
            .into_iter()
            .map(|cards| Reward::Card(CardReward::of_cards(cards, None)))
            .collect();
        ctx.offer_rewards(rewards);
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for Kaleidoscope {
    type Me = RelicId;
}
