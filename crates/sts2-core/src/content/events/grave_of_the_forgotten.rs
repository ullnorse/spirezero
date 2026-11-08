//! `GraveOfTheForgotten` (`Models.Events/GraveOfTheForgotten.cs`): take a `Decay` curse and
//! enchant a card with `SoulsPower`, or accept the `ForgottenSoul` relic. Only enters if the deck
//! has a card `SoulsPower` can enchant.

use crate::content::enchantments::EnchantmentKind;
use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::content::relics::RelicKind;
use crate::run::{RunError, RunState, SelectPurpose};

/// `CardSelectorPrefs(EnchantSelectionPrompt, 1)`.
const PICKS: usize = 1;
/// `CardCmd.Enchant<SoulsPower>(card, 1m)`.
const AMOUNT: i32 = 1;

pub struct GraveOfTheForgotten;

/// `HasEnchantableCards(player)`: a deck card `SoulsPower` can enchant. A deck card that is not
/// ported counts, as its stub room is refused either way.
fn has_enchantable_cards(s: &RunState) -> bool {
    s.deck
        .iter()
        .any(|c| c.can_enchant(EnchantmentKind::SoulsPower).unwrap_or(true))
}

impl EventModel for GraveOfTheForgotten {
    fn is_allowed(&self, s: &RunState) -> bool {
        has_enchantable_cards(s)
    }

    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let confront = if has_enchantable_cards(ctx.state()) {
            EventOption::new(
                "GRAVE_OF_THE_FORGOTTEN.pages.INITIAL.options.CONFRONT",
                "CONFRONT",
            )
        } else {
            EventOption::locked("GRAVE_OF_THE_FORGOTTEN.pages.INITIAL.options.CONFRONT_LOCKED")
        };
        Ok(vec![
            confront,
            EventOption::new(
                "GRAVE_OF_THE_FORGOTTEN.pages.INITIAL.options.ACCEPT",
                "ACCEPT",
            ),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if action.tag == "ACCEPT" {
            ctx.obtain_relic(RelicKind::ForgottenSoul.id())?;
        } else {
            // `AddCurseToDeck<Decay>`, then `FromDeckForEnchantment` (no card: nothing).
            let kind = EnchantmentKind::SoulsPower;
            ctx.state().add_card(crate::run::RunCard::new("DECAY"));
            let options = ctx.state().enchantable_cards(kind)?;
            let purpose = SelectPurpose::Enchant {
                kind,
                amount: AMOUNT,
            };
            ctx.select(purpose, options, PICKS)?;
        }
        Ok(Page::finished("GRAVE_OF_THE_FORGOTTEN", action.tag))
    }
}
