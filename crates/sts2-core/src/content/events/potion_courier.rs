//! `PotionCourier` (`Models.Events/PotionCourier.cs`): three `FoulPotion` rewards, or one
//! Uncommon potion reward. Shared, Act 2 and 3.

use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::rng::PlayerRngType;
use crate::run::odds::PotionRarity;
use crate::run::potions::potion_options;
use crate::run::{Reward, RunError, RunState};

/// `DynamicVar("FoulPotions", 3m)`.
const FOUL_POTIONS: usize = 3;

pub struct PotionCourier;

impl EventModel for PotionCourier {
    /// `CurrentActIndex > 0` (`PotionCourier.cs:31-34`).
    fn is_allowed(&self, s: &RunState) -> bool {
        s.act_index > 0
    }

    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(vec![
            EventOption::new("POTION_COURIER.pages.INITIAL.options.GRAB_POTIONS", "GRAB"),
            EventOption::new("POTION_COURIER.pages.INITIAL.options.RANSACK", "RANSACK"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if action.tag == "GRAB" {
            ctx.offer_rewards(vec![Reward::Potion("FOUL_POTION"); FOUL_POTIONS]);
        } else {
            // `Owner.PlayerRng.Rewards.NextItem` over the character then shared pool's Uncommon
            // potions.
            let potions: Vec<&'static str> = potion_options(ctx.state().character)
                .into_iter()
                .filter(|&(_, rarity)| rarity == PotionRarity::Uncommon)
                .map(|(id, _)| id)
                .collect();
            let rng = &mut ctx.state().player_rng[PlayerRngType::Rewards];
            if let Some(&potion) = rng.next_item(&potions) {
                ctx.offer_rewards(vec![Reward::Potion(potion)]);
            }
        }
        let page = if action.tag == "GRAB" {
            "GRAB_POTIONS"
        } else {
            "RANSACK"
        };
        Ok(Page::finished("POTION_COURIER", page))
    }
}
