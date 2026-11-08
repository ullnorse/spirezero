//! `AromaOfChaos` (`Models.Events/AromaOfChaos.cs`): transform a chosen card, or upgrade one.

use crate::combat::PromptText;
use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::run::{RunError, SelectPurpose};

/// `CardSelectorPrefs(.., 1)`.
const PICKS: usize = 1;

pub struct AromaOfChaos;

impl EventModel for AromaOfChaos {
    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(vec![
            EventOption::new("AROMA_OF_CHAOS.pages.INITIAL.options.LET_GO", "LET_GO"),
            EventOption::new(
                "AROMA_OF_CHAOS.pages.INITIAL.options.MAINTAIN_CONTROL",
                "MAINTAIN_CONTROL",
            ),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if action.tag == "LET_GO" {
            let options = ctx.state().transformable_cards();
            ctx.select_deck_cards(action, PromptText::Transform, options, PICKS)?;
        } else {
            let options = ctx.state().upgradable_cards();
            ctx.select(SelectPurpose::Upgrade, options, PICKS)?;
        }
        Ok(Page::finished("AROMA_OF_CHAOS", action.tag))
    }

    /// `LetGo` after `FromDeckForTransformation`: `TransformToRandom` on the event's `Rng`.
    fn on_cards_selected(
        &self,
        _action: EventAction,
        ctx: &mut EventCtx<'_>,
        cards: &[usize],
    ) -> Result<(), RunError> {
        for &i in cards {
            ctx.transform_to_random(i)?;
        }
        Ok(())
    }
}
