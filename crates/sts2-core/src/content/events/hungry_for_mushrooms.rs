//! `HungryForMushrooms` (`Models.Events/HungryForMushrooms.cs`): take the `BigMushroom` or the
//! `FragrantMushroom` (which costs 15 HP).

use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::content::relics::RelicKind;
use crate::run::RunError;

pub struct HungryForMushrooms;

impl EventModel for HungryForMushrooms {
    /// `RelicOption<T>`: the text key is `OptionKey("INITIAL", relic.Id.Entry)`.
    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(vec![
            EventOption::new(
                "HUNGRY_FOR_MUSHROOMS.pages.INITIAL.options.BIG_MUSHROOM",
                "BIG_MUSHROOM",
            ),
            EventOption::new(
                "HUNGRY_FOR_MUSHROOMS.pages.INITIAL.options.FRAGRANT_MUSHROOM",
                "FRAGRANT_MUSHROOM",
            ),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        let relic = if action.tag == "BIG_MUSHROOM" {
            RelicKind::BigMushroom
        } else {
            RelicKind::FragrantMushroom
        };
        ctx.obtain_relic(relic.id())?;
        Ok(Page::finished("HUNGRY_FOR_MUSHROOMS", action.tag))
    }
}
