//! `ZenWeaver` (`Models.Events/ZenWeaver.cs`): 50 gold for 2 `Enlightenment`s, 125 gold to remove
//! a card, or 250 gold to remove 2 (locked options below their cost).

use crate::combat::PromptText;
use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::run::{RunCard, RunError, RunState};

/// `BreathingTechniquesCost`.
const BREATHING_TECHNIQUES_COST: i32 = 50;
/// `EmotionalAwarenessCost`.
const EMOTIONAL_AWARENESS_COST: i32 = 125;
/// `ArachnidAcupunctureCost`.
const ARACHNID_ACUPUNCTURE_COST: i32 = 250;
/// The `Enlightenment`s Breathing Techniques adds.
const ENLIGHTENMENTS: usize = 2;

pub struct ZenWeaver;

/// The gold and removal count of a removal option.
fn removal(tag: &str) -> (i32, usize) {
    if tag == "EMOTIONAL_AWARENESS" {
        (EMOTIONAL_AWARENESS_COST, 1)
    } else {
        (ARACHNID_ACUPUNCTURE_COST, 2)
    }
}

impl EventModel for ZenWeaver {
    fn is_allowed(&self, s: &RunState) -> bool {
        s.gold >= EMOTIONAL_AWARENESS_COST
    }

    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let gold = ctx.state().gold;
        let option = |cost: i32, tag: &'static str| {
            if gold < cost {
                EventOption::locked("ZEN_WEAVER.pages.INITIAL.options.LOCKED")
            } else {
                EventOption::new(format!("ZEN_WEAVER.pages.INITIAL.options.{tag}"), tag)
            }
        };
        Ok(vec![
            EventOption::new(
                "ZEN_WEAVER.pages.INITIAL.options.BREATHING_TECHNIQUES",
                "BREATHING_TECHNIQUES",
            ),
            option(EMOTIONAL_AWARENESS_COST, "EMOTIONAL_AWARENESS"),
            option(ARACHNID_ACUPUNCTURE_COST, "ARACHNID_ACUPUNCTURE"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if action.tag == "BREATHING_TECHNIQUES" {
            let s = ctx.state();
            s.lose_gold(BREATHING_TECHNIQUES_COST);
            for _ in 0..ENLIGHTENMENTS {
                s.add_card(RunCard::new("ENLIGHTENMENT"));
            }
        } else {
            let options = ctx.state().removable_cards();
            ctx.select_deck_cards(action, PromptText::Remove, options, removal(action.tag).1)?;
        }
        Ok(Page::finished("ZEN_WEAVER", action.tag))
    }

    /// `RemoveCardsAndProceed` after `FromDeckForRemoval`: `RemoveFromDeck`, then the gold.
    fn on_cards_selected(
        &self,
        action: EventAction,
        ctx: &mut EventCtx<'_>,
        cards: &[usize],
    ) -> Result<(), RunError> {
        let s = ctx.state();
        s.remove_cards(cards);
        s.lose_gold(removal(action.tag).0);
        Ok(())
    }
}
