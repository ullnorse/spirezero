//! `InfestedAutomaton` (`Models.Events/InfestedAutomaton.cs`): add a random Power of the
//! character's pool, or a random card that costs 0 (not X), to the deck.

use crate::content::cards::CardType;
use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::run::{CardCreation, CardFilter, CardPool, RunError};

pub struct InfestedAutomaton;

impl EventModel for InfestedAutomaton {
    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(vec![
            EventOption::new("INFESTED_AUTOMATON.pages.INITIAL.options.STUDY", "STUDY"),
            EventOption::new(
                "INFESTED_AUTOMATON.pages.INITIAL.options.TOUCH_CORE",
                "TOUCH_CORE",
            ),
        ])
    }

    /// `CardFactory.CreateForReward(Owner, 1, ForNonCombatWithDefaultOdds(..))`, then the card
    /// joins the deck.
    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        let options = if action.tag == "STUDY" {
            CardCreation {
                filter: CardFilter::Type(CardType::Power),
                ..CardCreation::non_combat_default(CardPool::Character)
            }
        } else {
            CardCreation {
                filter: CardFilter::ZeroCost,
                ..CardCreation::non_combat_default(CardPool::Character)
            }
            .without_pool_modifications()
        };
        for card in ctx.create_cards(1, options) {
            ctx.state().add_card(card);
        }
        Ok(Page::finished("INFESTED_AUTOMATON", action.tag))
    }
}
