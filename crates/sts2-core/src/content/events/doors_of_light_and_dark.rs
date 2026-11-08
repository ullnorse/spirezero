//! `DoorsOfLightAndDark` (`Models.Events/DoorsOfLightAndDark.cs`): upgrade 2 random cards, or
//! remove a chosen one.

use crate::combat::PromptText;
use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::run::RunError;
use crate::shuffle::stable_shuffle_by;

/// `CardsVar(2)`.
const UPGRADES: usize = 2;
/// `CardSelectorPrefs(RemoveSelectionPrompt, 1)`.
const REMOVALS: usize = 1;

pub struct DoorsOfLightAndDark;

impl EventModel for DoorsOfLightAndDark {
    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(vec![
            EventOption::new(
                "DOORS_OF_LIGHT_AND_DARK.pages.INITIAL.options.LIGHT",
                "LIGHT",
            ),
            EventOption::new("DOORS_OF_LIGHT_AND_DARK.pages.INITIAL.options.DARK", "DARK"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if action.tag == "LIGHT" {
            // The upgradable deck cards, `StableShuffle(Rng)` (sorted by `CardModel.CompareTo`:
            // id, then upgrade level), the first two upgraded.
            let mut cards = ctx.state().upgradable_cards();
            let deck = ctx.state().deck.clone();
            stable_shuffle_by(&mut cards, ctx.rng, |&a, &b| {
                deck[a]
                    .id
                    .cmp(deck[b].id)
                    .then(deck[a].upgrade_level.cmp(&deck[b].upgrade_level))
            });
            for &i in cards.iter().take(UPGRADES) {
                ctx.state().deck[i].upgrade();
            }
        } else {
            let options = ctx.state().removable_cards();
            ctx.select_deck_cards(action, PromptText::Remove, options, REMOVALS)?;
        }
        Ok(Page::finished("DOORS_OF_LIGHT_AND_DARK", action.tag))
    }

    /// `Dark` after `FromDeckForRemoval`: `RemoveFromDeck`.
    fn on_cards_selected(
        &self,
        _action: EventAction,
        ctx: &mut EventCtx<'_>,
        cards: &[usize],
    ) -> Result<(), RunError> {
        ctx.state().remove_cards(cards);
        Ok(())
    }
}
