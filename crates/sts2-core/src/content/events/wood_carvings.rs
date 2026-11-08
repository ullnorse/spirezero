//! `WoodCarvings` (`Models.Events/WoodCarvings.cs`): turn a chosen Basic card into a `Peck` or a
//! `ToricToughness`, or enchant a card with `Slither`.

use crate::combat::PromptText;
use crate::content::enchantments::EnchantmentKind;
use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::run::{RunCard, RunError, RunState, SelectPurpose};

/// `CardSelectorPrefs(.., 1)`.
const PICKS: usize = 1;
/// `Slither` is enchanted with `1m`.
const SNAKE_AMOUNT: i32 = 1;

pub struct WoodCarvings;

/// The deck cards `c.IsTransformable && c.Rarity == CardRarity.Basic`, in deck order.
fn transformable_basics(s: &RunState) -> Vec<usize> {
    (0..s.deck.len())
        .filter(|&i| s.deck[i].is_basic() && s.deck[i].is_removable())
        .collect()
}

impl EventModel for WoodCarvings {
    /// A Basic card that is removable.
    fn is_allowed(&self, s: &RunState) -> bool {
        s.deck.iter().any(|c| c.is_basic() && c.is_removable())
    }

    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let snake = if ctx
            .state()
            .enchantable_cards(EnchantmentKind::Slither)?
            .is_empty()
        {
            EventOption::locked("WOOD_CARVINGS.pages.INITIAL.options.SNAKE_LOCKED")
        } else {
            EventOption::new("WOOD_CARVINGS.pages.INITIAL.options.SNAKE", "SNAKE")
        };
        Ok(vec![
            EventOption::new("WOOD_CARVINGS.pages.INITIAL.options.BIRD", "BIRD"),
            snake,
            EventOption::new("WOOD_CARVINGS.pages.INITIAL.options.TORUS", "TORUS"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if action.tag == "SNAKE" {
            let kind = EnchantmentKind::Slither;
            let options = ctx.state().enchantable_cards(kind)?;
            let purpose = SelectPurpose::Enchant {
                kind,
                amount: SNAKE_AMOUNT,
            };
            ctx.select(purpose, options, PICKS)?;
        } else {
            let options = transformable_basics(ctx.state());
            ctx.select_deck_cards(action, PromptText::Transform, options, PICKS)?;
        }
        Ok(Page::finished("WOOD_CARVINGS", action.tag))
    }

    /// `Bird` / `Torus` after `FromDeckGeneric`: `TransformTo<Peck>` / `<ToricToughness>`.
    fn on_cards_selected(
        &self,
        action: EventAction,
        ctx: &mut EventCtx<'_>,
        cards: &[usize],
    ) -> Result<(), RunError> {
        let id = if action.tag == "BIRD" {
            "PECK"
        } else {
            "TORIC_TOUGHNESS"
        };
        let replacement = RunCard::new(id);
        if replacement.to_combat().is_none() {
            return Err(RunError::UnsupportedCard(id));
        }
        for &i in cards {
            ctx.state().transform_to(i, replacement);
        }
        Ok(())
    }
}
