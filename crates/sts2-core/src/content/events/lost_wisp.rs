//! `LostWisp` (`Models.Events/LostWisp.cs`): take a `Decay` and the `LostWisp` relic, or search
//! for 60 ± 15 gold.

use crate::content::events::{EventAction, EventCtx, EventData, EventModel, EventOption, Page};
use crate::decimal::Decimal;
use crate::run::{RunCard, RunError};

/// `_baseGold`.
const BASE_GOLD: i32 = 60;
/// `_goldVariance`: `NextInt(-15, 16)`.
const GOLD_VARIANCE: i32 = 15;

pub struct LostWisp;

impl EventModel for LostWisp {
    /// `CalculateVars`: `Gold += Rng.NextInt(-15, 16)`.
    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let gold = BASE_GOLD + ctx.rng.next_int_range(-GOLD_VARIANCE, GOLD_VARIANCE + 1);
        *ctx.data = EventData::Amount(Decimal::from(gold));
        Ok(vec![
            EventOption::new("LOST_WISP.pages.INITIAL.options.CLAIM", "CLAIM"),
            EventOption::new("LOST_WISP.pages.INITIAL.options.SEARCH", "SEARCH"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if action.tag == "CLAIM" {
            ctx.state().add_card(RunCard::new("DECAY"));
            ctx.obtain_relic("LOST_WISP")?;
        } else {
            let EventData::Amount(gold) = *ctx.data else {
                unreachable!("LostWisp's gold is set in begin");
            };
            ctx.state().gain_gold(gold);
        }
        Ok(Page::finished("LOST_WISP", action.tag))
    }
}
