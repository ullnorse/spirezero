//! `TheLegendsWereTrue` (`Models.Events/TheLegendsWereTrue.cs`): a `SpoilsMap`, or 8 damage for
//! a random potion.

use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::rng::PlayerRngType;
use crate::run::potions::potion_options;
use crate::run::rewards::Reward;
use crate::run::{RunCard, RunError, RunState};

/// `DamageVar(8m, Unblockable | Unpowered)`.
const DAMAGE: i32 = 8;

pub struct TheLegendsWereTrue;

impl EventModel for TheLegendsWereTrue {
    /// Act 1, a non-empty deck and at least 10 HP.
    fn is_allowed(&self, s: &RunState) -> bool {
        s.act_index == 0 && !s.deck.is_empty() && s.hp >= 10
    }

    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(vec![
            EventOption::new(
                "THE_LEGENDS_WERE_TRUE.pages.INITIAL.options.NAB_THE_MAP",
                "NAB_THE_MAP",
            ),
            EventOption::new(
                "THE_LEGENDS_WERE_TRUE.pages.INITIAL.options.SLOWLY_FIND_AN_EXIT",
                "SLOWLY_FIND_AN_EXIT",
            ),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if action.tag == "NAB_THE_MAP" {
            ctx.state().add_card(RunCard::new("SPOILS_MAP"));
        } else {
            // `SlowlyFindAnExit`: the damage, then one uniform `NextItem` on the player's
            // `rewards` over the unlocked character and shared potions, offered on its own.
            ctx.state().damage(DAMAGE);
            let potions: Vec<&'static str> = potion_options(ctx.state().character)
                .into_iter()
                .map(|(id, _)| id)
                .collect();
            let rng = &mut ctx.state().player_rng[PlayerRngType::Rewards];
            let potion = *rng
                .next_item(&potions)
                .expect("the potion pools are not empty");
            ctx.offer_rewards(vec![Reward::Potion(potion)]);
        }
        Ok(Page::finished("THE_LEGENDS_WERE_TRUE", action.tag))
    }
}
