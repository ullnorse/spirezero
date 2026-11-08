//! `ByrdonisNest` (`Models.Events/ByrdonisNest.cs`): +7 max HP, or a `ByrdonisEgg`.

use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::run::{RunCard, RunError, RunState};

/// `MaxHpVar(7m)`.
const MAX_HP: i32 = 7;
/// The relics with `AddsPet`.
const PET_RELICS: [&str; 2] = ["BYRDPIP", "PAELS_LEGION"];

pub struct ByrdonisNest;

/// `Player.HasEventPet()` (`Player.cs:248-255`): a pet relic, or a `ByrdonisEgg` in the deck.
#[must_use]
pub fn has_event_pet(s: &RunState) -> bool {
    s.relics.iter().any(|r| PET_RELICS.contains(&r.id()))
        || s.deck.iter().any(|c| c.id == "BYRDONIS_EGG")
}

impl EventModel for ByrdonisNest {
    /// `!Player.HasEventPet()`.
    fn is_allowed(&self, s: &RunState) -> bool {
        !has_event_pet(s)
    }

    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(vec![
            EventOption::new("BYRDONIS_NEST.pages.INITIAL.options.EAT", "EAT"),
            EventOption::new("BYRDONIS_NEST.pages.INITIAL.options.TAKE", "TAKE"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        match action.tag {
            "EAT" => ctx.state().gain_max_hp(MAX_HP),
            _ => ctx.state().add_card(RunCard::new("BYRDONIS_EGG")),
        }
        Ok(Page::finished("BYRDONIS_NEST", action.tag))
    }
}
