//! `FieldOfManSizedHoles` (`Models.Events/FieldOfManSizedHoles.cs`): remove 2 cards and take a
//! `Normality`, or enchant a card with `PerfectFit`.

use crate::combat::PromptText;
use crate::content::enchantments::EnchantmentKind;
use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::run::{RunCard, RunError, RunState, SelectPurpose};

/// `CardsVar(2)`: the cards Resist removes.
const RESIST_REMOVALS: usize = 2;
/// `CardSelectorPrefs(EnchantSelectionPrompt, 1)`.
const ENCHANT_PICKS: usize = 1;
/// `CardCmd.Enchant<PerfectFit>(card, 1m)`.
const PERFECT_FIT_AMOUNT: i32 = 1;

pub struct FieldOfManSizedHoles;

impl EventModel for FieldOfManSizedHoles {
    /// A deck card `PerfectFit` can enchant. A deck holding an unported card is refused here, as
    /// it could start no fight anyway.
    fn is_allowed(&self, s: &RunState) -> bool {
        s.enchantable_cards(EnchantmentKind::PerfectFit)
            .is_ok_and(|cards| !cards.is_empty())
    }

    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(vec![
            EventOption::new(
                "FIELD_OF_MAN_SIZED_HOLES.pages.INITIAL.options.RESIST",
                "RESIST",
            ),
            EventOption::new(
                "FIELD_OF_MAN_SIZED_HOLES.pages.INITIAL.options.ENTER_YOUR_HOLE",
                "ENTER_YOUR_HOLE",
            ),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if action.tag == "RESIST" {
            let options = ctx.state().removable_cards();
            ctx.select_deck_cards(action, PromptText::Remove, options, RESIST_REMOVALS)?;
        } else {
            let kind = EnchantmentKind::PerfectFit;
            let options = ctx.state().enchantable_cards(kind)?;
            let purpose = SelectPurpose::Enchant {
                kind,
                amount: PERFECT_FIT_AMOUNT,
            };
            ctx.select(purpose, options, ENCHANT_PICKS)?;
        }
        Ok(Page::finished("FIELD_OF_MAN_SIZED_HOLES", action.tag))
    }

    /// `Resist` after `FromDeckForRemoval`: `RemoveFromDeck`, then `AddCursesToDeck(Normality)`.
    fn on_cards_selected(
        &self,
        _action: EventAction,
        ctx: &mut EventCtx<'_>,
        cards: &[usize],
    ) -> Result<(), RunError> {
        let s = ctx.state();
        s.remove_cards(cards);
        s.add_card(RunCard::new("NORMALITY"));
        Ok(())
    }
}
