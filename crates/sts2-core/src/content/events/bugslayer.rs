//! `Bugslayer` (`Models.Events/Bugslayer.cs`): add an `Exterminate` or a `Squash` to the deck.

use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::run::{RunCard, RunError};

pub struct Bugslayer;

impl EventModel for Bugslayer {
    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(vec![
            EventOption::new(
                "BUGSLAYER.pages.INITIAL.options.EXTERMINATION",
                "EXTERMINATION",
            ),
            EventOption::new("BUGSLAYER.pages.INITIAL.options.SQUASH", "SQUASH"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        let card = if action.tag == "EXTERMINATION" {
            "EXTERMINATE"
        } else {
            "SQUASH"
        };
        ctx.state().add_card(RunCard::new(card));
        Ok(Page::finished("BUGSLAYER", action.tag))
    }
}
