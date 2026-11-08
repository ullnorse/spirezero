//! `SlipperyBridge` (`Models.Events/SlipperyBridge.cs`): lose a random card, or hold on for
//! growing damage and a new random card.

use crate::content::events::{EventAction, EventCtx, EventData, EventModel, EventOption, Page};
use crate::run::{RunError, RunState};

/// `_initialHpLoss`.
const INITIAL_HP_LOSS: i32 = 3;
const OVERCOME_KEY: &str = "SLIPPERY_BRIDGE.pages.INITIAL.options.OVERCOME";

pub struct SlipperyBridge;

impl SlipperyBridge {
    /// `GetNewRandomCard()` on the event `Rng`. The first card is a removable non-Basic one;
    /// later ones skip every card offered before and every copy of the last one. With no such
    /// card, any removable card.
    fn new_random_card(
        ctx: &mut EventCtx<'_>,
        previous: Option<usize>,
        skipped: &[usize],
    ) -> usize {
        let deck = ctx.state().deck.clone();
        let candidates = |keep: &dyn Fn(usize) -> bool| -> Vec<usize> {
            (0..deck.len())
                .filter(|&i| keep(i) && deck[i].is_removable() && !skipped.contains(&i))
                .collect()
        };
        let mut list = previous.map_or_else(
            || candidates(&|i| !deck[i].is_basic()),
            |p| candidates(&|i| deck[i].id != deck[p].id),
        );
        if list.is_empty() {
            list = (0..deck.len())
                .filter(|&i| deck[i].is_removable())
                .collect();
        }
        *ctx.rng
            .next_item(&list)
            .expect("IsAllowed guarantees a removable card")
    }

    /// `GetHoldOnSuffix`.
    fn suffix(hold_ons: i32) -> String {
        if hold_ons >= 7 {
            "LOOP".to_owned()
        } else {
            hold_ons.to_string()
        }
    }
}

impl EventModel for SlipperyBridge {
    /// `TotalFloor > 6` and a removable card in the deck.
    fn is_allowed(&self, s: &RunState) -> bool {
        s.total_floor() > 6 && s.deck.iter().any(|c| c.is_removable())
    }

    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let card = Self::new_random_card(ctx, None, &[]);
        *ctx.data = EventData::SlipperyBridge {
            hold_ons: 0,
            card,
            skipped: Vec::new(),
        };
        Ok(vec![
            EventOption::new(OVERCOME_KEY, "OVERCOME"),
            EventOption::new("SLIPPERY_BRIDGE.pages.INITIAL.options.HOLD_ON_0", "HOLD_ON"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        let EventData::SlipperyBridge {
            hold_ons,
            card,
            mut skipped,
        } = ctx.data.clone()
        else {
            unreachable!("SlipperyBridge's vars are set in begin");
        };
        if action.tag == "OVERCOME" {
            ctx.state().remove_card(card);
            return Ok(Page::finished("SLIPPERY_BRIDGE", "OVERCOME"));
        }
        // `HoldOn`: `CurrentHpLoss` is `3 + NumberOfHoldOns`.
        ctx.state().damage(INITIAL_HP_LOSS + hold_ons);
        let hold_ons = hold_ons + 1;
        skipped.push(card);
        let card = Self::new_random_card(ctx, Some(card), &skipped);
        *ctx.data = EventData::SlipperyBridge {
            hold_ons,
            card,
            skipped,
        };
        let (from, to) = (Self::suffix(hold_ons - 1), Self::suffix(hold_ons));
        Ok(Page::Next(vec![
            EventOption::new(OVERCOME_KEY, "OVERCOME"),
            EventOption::new(
                format!("SLIPPERY_BRIDGE.pages.HOLD_ON_{from}.options.HOLD_ON_{to}"),
                "HOLD_ON",
            ),
        ]))
    }
}
