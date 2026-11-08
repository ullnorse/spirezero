//! `Symbiote` (`Models.Events/Symbiote.cs`): enchant an attack with `Corrupted`, or transform a
//! card on the event `Rng`. Shared, Act 2 and 3.

use crate::combat::PromptText;
use crate::content::enchantments::EnchantmentKind;
use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::run::{RunError, RunState, SelectPurpose};

/// `Corrupted` is enchanted with `1`.
const CORRUPTED_AMOUNT: i32 = 1;
/// `CardSelectorPrefs(EnchantSelectionPrompt, 1)`.
const ENCHANTS: usize = 1;
/// `CardsVar(1)`: Kill With Fire transforms one card.
const TRANSFORMS: usize = 1;

pub struct Symbiote;

impl EventModel for Symbiote {
    /// `CurrentActIndex > 0` (`Symbiote.cs:31-34`).
    fn is_allowed(&self, s: &RunState) -> bool {
        s.act_index > 0
    }

    /// Approach is locked when no deck card can take `Corrupted`.
    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let approach = if ctx
            .state()
            .enchantable_cards(EnchantmentKind::Corrupted)?
            .is_empty()
        {
            EventOption::locked("SYMBIOTE.pages.INITIAL.options.APPROACH_LOCKED")
        } else {
            EventOption::new("SYMBIOTE.pages.INITIAL.options.APPROACH", "APPROACH")
        };
        Ok(vec![
            approach,
            EventOption::new(
                "SYMBIOTE.pages.INITIAL.options.KILL_WITH_FIRE",
                "KILL_WITH_FIRE",
            ),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if action.tag == "APPROACH" {
            let kind = EnchantmentKind::Corrupted;
            let options = ctx.state().enchantable_cards(kind)?;
            let purpose = SelectPurpose::Enchant {
                kind,
                amount: CORRUPTED_AMOUNT,
            };
            ctx.select(purpose, options, ENCHANTS)?;
        } else {
            let options = ctx.state().transformable_cards();
            ctx.select_deck_cards(action, PromptText::Transform, options, TRANSFORMS)?;
        }
        Ok(Page::finished("SYMBIOTE", action.tag))
    }

    /// Kill With Fire after `FromDeckForTransformation`: `TransformToRandom(card, Rng)`.
    fn on_cards_selected(
        &self,
        _action: EventAction,
        ctx: &mut EventCtx<'_>,
        cards: &[usize],
    ) -> Result<(), RunError> {
        for (n, &i) in cards.iter().enumerate() {
            let shift = cards[..n].iter().filter(|&&j| j < i).count();
            ctx.transform_to_random(i - shift)?;
        }
        Ok(())
    }
}
