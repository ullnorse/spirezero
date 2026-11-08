//! `RoomFullOfCheese` (`Models.Events/RoomFullOfCheese.cs`): pick 2 of 8 common cards, or take
//! 14 damage for `ChosenCheese`.

use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::run::odds::CardRarity;
use crate::run::rewards::{CardCreation, CardFilter, CardPool};
use crate::run::{RunError, RunState};

/// `DamageVar(14m, Unblockable | Unpowered)`.
const DAMAGE: i32 = 14;
/// `CreateForReward(owner, 8, ...)`.
const CARDS_SHOWN: usize = 8;
/// `CardSelectorPrefs(..., 2)`.
const CARDS_TAKEN: usize = 2;

pub struct RoomFullOfCheese;

impl EventModel for RoomFullOfCheese {
    fn is_allowed(&self, s: &RunState) -> bool {
        s.act_index < 2
    }

    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(vec![
            EventOption::new("ROOM_FULL_OF_CHEESE.pages.INITIAL.options.GORGE", "GORGE"),
            EventOption::new("ROOM_FULL_OF_CHEESE.pages.INITIAL.options.SEARCH", "SEARCH"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if action.tag == "GORGE" {
            // `Gorge`: 8 uniform commons of the character pool, pick 2 for the deck.
            let options = CardCreation::non_combat_uniform(
                CardPool::Character,
                CardFilter::Rarity(CardRarity::Common),
            );
            let cards = ctx.create_cards(CARDS_SHOWN, options);
            ctx.select_cards_to_add(cards, CARDS_TAKEN);
        } else {
            // `Search`.
            ctx.state().damage(DAMAGE);
            ctx.obtain_relic("CHOSEN_CHEESE")?;
        }
        Ok(Page::finished("ROOM_FULL_OF_CHEESE", action.tag))
    }
}
