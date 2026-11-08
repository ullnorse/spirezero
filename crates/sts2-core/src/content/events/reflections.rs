//! `Reflections` (`Models.Events/Reflections.cs`): downgrade 2 random upgraded cards and upgrade
//! 4 random upgradable ones (on the event's `Rng`), or shatter the mirror: clone every card in
//! the deck and take a `BadLuck`.

use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::run::{RunCard, RunError};

/// The loop bounds of `TouchAMirror`.
const DOWNGRADES: usize = 2;
const UPGRADES: usize = 4;

pub struct Reflections;

/// `for (i < times) { card = Rng.NextItem(cards); cards.Remove(card); apply(card) }`, stopping
/// when the list runs out.
fn pick_each(ctx: &mut EventCtx<'_>, mut cards: Vec<usize>, times: usize, apply: fn(&mut RunCard)) {
    for _ in 0..times {
        let Some(&card) = ctx.rng.next_item(&cards) else {
            break;
        };
        cards.retain(|&c| c != card);
        apply(&mut ctx.state().deck[card]);
    }
}

impl EventModel for Reflections {
    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(vec![
            EventOption::new(
                "REFLECTIONS.pages.INITIAL.options.TOUCH_A_MIRROR",
                "TOUCH_A_MIRROR",
            ),
            EventOption::new("REFLECTIONS.pages.INITIAL.options.SHATTER", "SHATTER"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if action.tag == "TOUCH_A_MIRROR" {
            let upgraded: Vec<usize> = (0..ctx.state().deck.len())
                .filter(|&i| ctx.state().deck[i].upgrade_level > 0)
                .collect();
            pick_each(ctx, upgraded, DOWNGRADES, RunCard::downgrade);
            // The list of upgradable cards is read after the downgrades.
            let upgradable = ctx.state().upgradable_cards();
            pick_each(ctx, upgradable, UPGRADES, RunCard::upgrade);
        } else {
            // `CloneCard(Owner.Deck.Cards[i])` for the original deck size, each added to the deck.
            let s = ctx.state();
            for i in 0..s.deck.len() {
                s.add_card(s.deck[i]);
            }
            s.add_card(RunCard::new("BAD_LUCK"));
        }
        Ok(Page::finished("REFLECTIONS", action.tag))
    }
}
