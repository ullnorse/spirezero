//! `Amalgamator` (`Models.Events/Amalgamator.cs`): remove 2 Basic Strikes for an `UltimateStrike`,
//! or 2 Basic Defends for an `UltimateDefend`.

use crate::combat::PromptText;
use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::run::{RunCard, RunError, RunState};

/// `CardSelectorPrefs(RemoveSelectionPrompt, 2)`, and the least `IsAllowed` needs of each.
const PICKS: usize = 2;

pub struct Amalgamator;

/// The deck cards `IsValid(tag, c)` (the tag, `Rarity` Basic, `IsRemovable`), in deck order.
fn valid(s: &RunState, strikes: bool) -> Vec<usize> {
    (0..s.deck.len())
        .filter(|&i| {
            let c = s.deck[i];
            (if strikes {
                c.is_strike()
            } else {
                c.is_defend()
            }) && c.is_removable()
        })
        .collect()
}

impl EventModel for Amalgamator {
    /// At least 2 valid Strikes and 2 valid Defends.
    fn is_allowed(&self, s: &RunState) -> bool {
        valid(s, true).len() >= PICKS && valid(s, false).len() >= PICKS
    }

    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(vec![
            EventOption::new(
                "AMALGAMATOR.pages.INITIAL.options.COMBINE_STRIKES",
                "COMBINE_STRIKES",
            ),
            EventOption::new(
                "AMALGAMATOR.pages.INITIAL.options.COMBINE_DEFENDS",
                "COMBINE_DEFENDS",
            ),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        let options = valid(ctx.state(), action.tag == "COMBINE_STRIKES");
        ctx.select_deck_cards(action, PromptText::Remove, options, PICKS)?;
        Ok(Page::finished("AMALGAMATOR", action.tag))
    }

    /// After `FromDeckForRemoval`: `RemoveFromDeck`, then the new card joins the deck.
    fn on_cards_selected(
        &self,
        action: EventAction,
        ctx: &mut EventCtx<'_>,
        cards: &[usize],
    ) -> Result<(), RunError> {
        let card = if action.tag == "COMBINE_STRIKES" {
            "ULTIMATE_STRIKE"
        } else {
            "ULTIMATE_DEFEND"
        };
        let s = ctx.state();
        s.remove_cards(cards);
        s.add_card(RunCard::new(card));
        Ok(())
    }
}
