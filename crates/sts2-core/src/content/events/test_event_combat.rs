//! Test-only event exercising `EnterCombatWithoutExitingEvent`: `FIGHT` starts a `SlimesWeak`
//! fight with 7 extra gold and comes back to a second page; `AMBUSH` starts one that goes to the
//! map afterwards.

#![cfg(test)]

use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::run::{PendingReward, Reward, RunError};

/// Test-only: an option that starts a fight in the event's room.
pub struct TestEventCombat;

impl EventModel for TestEventCombat {
    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(vec![
            EventOption::new("TEST_EVENT_COMBAT.FIGHT", "FIGHT"),
            EventOption::new("TEST_EVENT_COMBAT.AMBUSH", "AMBUSH"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        match action.tag {
            "FIGHT" => {
                ctx.enter_combat(
                    "SLIMES_WEAK",
                    vec![PendingReward::Ready(Reward::Gold(7))],
                    true,
                )?;
                Ok(Page::Next(vec![EventOption::new(
                    "TEST_EVENT_COMBAT.AFTER",
                    "AFTER",
                )]))
            }
            "AMBUSH" => {
                ctx.enter_combat("SLIMES_WEAK", Vec::new(), false)?;
                Ok(Page::left())
            }
            _ => Ok(Page::left()),
        }
    }
}
