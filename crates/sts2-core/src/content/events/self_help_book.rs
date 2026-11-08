//! `SelfHelpBook` (`Models.Events/SelfHelpBook.cs`): enchant an Attack with `Sharp`, a Skill
//! with `Nimble` or a Power with `Swift` (2 each); an option is locked if no deck card of its
//! type can take its enchantment.

use crate::content::cards::CardType;
use crate::content::enchantments::EnchantmentKind;
use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::run::{RunError, RunState, SelectPurpose};

/// `_sharpAmount`, `_nimbleAmount` and `_swiftAmount`.
const AMOUNT: i32 = 2;
/// `CardSelectorPrefs(.., 1)`.
const PICKS: usize = 1;

pub struct SelfHelpBook;

/// The enchantment, card type and tag of each reading, in option order.
const READINGS: [(EnchantmentKind, CardType, &str); 3] = [
    (EnchantmentKind::Sharp, CardType::Attack, "READ_THE_BACK"),
    (EnchantmentKind::Nimble, CardType::Skill, "READ_PASSAGE"),
    (EnchantmentKind::Swift, CardType::Power, "READ_ENTIRE_BOOK"),
];

/// The deck cards that can take `kind` and have `card_type` (`DeckFilter`), in deck order.
fn candidates(
    s: &RunState,
    kind: EnchantmentKind,
    card_type: CardType,
) -> Result<Vec<usize>, RunError> {
    let mut cards = s.enchantable_cards(kind)?;
    cards.retain(|&i| s.deck[i].card_type() == Some(card_type));
    Ok(cards)
}

impl EventModel for SelfHelpBook {
    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let mut available = Vec::new();
        for (kind, card_type, _) in READINGS {
            available.push(!candidates(ctx.state(), kind, card_type)?.is_empty());
        }
        if !available.contains(&true) {
            return Ok(vec![EventOption::new(
                "SELF_HELP_BOOK.pages.INITIAL.options.NO_OPTIONS",
                "NO_OPTIONS",
            )]);
        }
        Ok(READINGS
            .iter()
            .zip(available)
            .map(|(&(_, _, tag), available)| {
                let key = format!("SELF_HELP_BOOK.pages.INITIAL.options.{tag}");
                if available {
                    EventOption::new(key, tag)
                } else {
                    EventOption::locked(format!("{key}_LOCKED"))
                }
            })
            .collect())
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if let Some(&(kind, card_type, _)) = READINGS.iter().find(|r| r.2 == action.tag) {
            let options = candidates(ctx.state(), kind, card_type)?;
            let purpose = SelectPurpose::Enchant {
                kind,
                amount: AMOUNT,
            };
            ctx.select(purpose, options, PICKS)?;
        }
        Ok(Page::finished("SELF_HELP_BOOK", action.tag))
    }
}
