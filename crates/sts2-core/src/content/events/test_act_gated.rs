//! Test-only event with an act gate, as the shared events have (`DollRoom.IsAllowed`:
//! `CurrentActIndex == 1`): it shows that an act-gated event's predicate lives in its own file.

#![cfg(test)]

use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::run::{RunError, RunState};

/// Test-only: an event allowed only in the second act.
pub struct TestActGated;

impl EventModel for TestActGated {
    fn is_allowed(&self, s: &RunState) -> bool {
        s.act_index == 1
    }

    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(vec![EventOption::new("TEST_ACT_GATED.LEAVE", "LEAVE")])
    }

    fn choose(&self, _action: EventAction, _ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        Ok(Page::left())
    }
}
