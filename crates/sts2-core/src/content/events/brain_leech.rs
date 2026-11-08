//! `BrainLeech` (`Models.Events/BrainLeech.cs`): pick 1 of 5 character cards, or take 5 damage
//! for a colorless card reward.

use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::run::rewards::{CardCreation, CardPool};
use crate::run::{RunError, RunState};

/// `DamageVar("RipHpLoss", 5m, Unblockable | Unpowered)`.
const RIP_HP_LOSS: i32 = 5;
/// `IntVar("RewardCount", 1m)`.
const REWARD_COUNT: usize = 1;
/// `new CardReward(options, 3, Owner)`.
const REWARD_CARDS: usize = 3;
/// `IntVar("FromCardChoiceCount", 5m)`.
const CARDS_SHOWN: usize = 5;
/// `CardSelectorPrefs(..., 1)`.
const CARDS_TAKEN: usize = 1;

pub struct BrainLeech;

impl EventModel for BrainLeech {
    fn is_allowed(&self, s: &RunState) -> bool {
        s.act_index < 2
    }

    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(vec![
            EventOption::new(
                "BRAIN_LEECH.pages.INITIAL.options.SHARE_KNOWLEDGE",
                "SHARE_KNOWLEDGE",
            ),
            EventOption::new("BRAIN_LEECH.pages.INITIAL.options.RIP", "RIP"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if action.tag == "SHARE_KNOWLEDGE" {
            // `ShareKnowledge`: 5 cards at the default non-combat odds, pick 1 for the deck.
            let options = CardCreation::non_combat_default(CardPool::Character);
            let cards = ctx.create_cards(CARDS_SHOWN, options);
            ctx.select_cards_to_add(cards, CARDS_TAKEN);
        } else {
            // `Rip`: the damage, then one colorless `CardReward` offered on its own. The card
            // reward is created when the rewards screen populates it, which `RewardsSet.Offer`
            // skips for a dead player.
            ctx.state().damage(RIP_HP_LOSS);
            if ctx.state().hp <= 0 {
                return Ok(Page::finished("BRAIN_LEECH", "RIP"));
            }
            let options = CardCreation::non_combat_default(CardPool::Colorless)
                .without_pool_modifications()
                .as_card_reward();
            let mut rewards = Vec::with_capacity(REWARD_COUNT);
            for _ in 0..REWARD_COUNT {
                rewards.push(ctx.card_reward(REWARD_CARDS, options));
            }
            ctx.offer_rewards(rewards);
        }
        Ok(Page::finished("BRAIN_LEECH", action.tag))
    }
}
