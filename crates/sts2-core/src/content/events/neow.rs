//! `Neow` (`Models.Events/Neow.cs`): the Act 1 ancient. The run layer plays it as its own room
//! (`run/neow.rs`, `Room::Neow`) and never picks it from an act's event list, so this model only
//! registers it with the event kinds; its options are not an `EventOption` page.

use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::run::{RunError, RunState};

pub struct Neow;

impl EventModel for Neow {
    /// An ancient is never in an act's event list, so it is never pulled as a `?` event.
    fn is_allowed(&self, _s: &RunState) -> bool {
        false
    }

    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Err(RunError::IllegalAction)
    }

    fn choose(&self, _action: EventAction, _ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        Err(RunError::IllegalAction)
    }
}
