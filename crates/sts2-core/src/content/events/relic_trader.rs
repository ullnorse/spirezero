//! `RelicTrader` (`Models.Events/RelicTrader.cs`): swap one of three random tradable relics for
//! the relic offered beside it. Shared, Act 2 and 3.

use crate::content::events::{EventAction, EventCtx, EventData, EventModel, EventOption, Page};
use crate::run::relics::is_tradable;
use crate::run::{RunError, RunState};
use crate::shuffle::stable_shuffle_by;

/// The trades offered (`Take(3)`).
const TRADES: usize = 3;
/// The tradable relics `IsAllowed` needs.
const MIN_TRADABLE: usize = 5;
/// `TOP`, `MIDDLE`, `BOTTOM`: the option of each trade.
const OPTIONS: [&str; TRADES] = [
    "RELIC_TRADER.pages.INITIAL.options.TOP",
    "RELIC_TRADER.pages.INITIAL.options.MIDDLE",
    "RELIC_TRADER.pages.INITIAL.options.BOTTOM",
];

pub struct RelicTrader;

/// `GetValidRelics`: the indices of the owned `IsTradable` relics, in relic order.
fn tradable_relics(s: &RunState) -> Vec<usize> {
    (0..s.relics.len())
        .filter(|&i| is_tradable(&s.relics[i]))
        .collect()
}

impl EventModel for RelicTrader {
    /// Not Act 1 and at least 5 tradable relics (`RelicTrader.cs:98-105`).
    fn is_allowed(&self, s: &RunState) -> bool {
        s.act_index != 0 && tradable_relics(s).len() >= MIN_TRADABLE
    }

    /// `CalculateVars` reads `OwnedRelics` (the tradable relics `StableShuffle`d on the event
    /// `Rng`, sorted by id first, then the first 3), then, if there is one, `NewRelics` (3 ×
    /// `PullNextRelicFromFront`).
    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let mut owned = tradable_relics(ctx.state());
        let relics = ctx.state().relics.clone();
        stable_shuffle_by(&mut owned, ctx.rng, |&a, &b| {
            relics[a].id().cmp(relics[b].id())
        });
        owned.truncate(TRADES);
        let new = if owned.is_empty() {
            Vec::new()
        } else {
            (0..TRADES).map(|_| ctx.pull_relic_from_front()).collect()
        };
        let mut options: Vec<EventOption> = OPTIONS
            .iter()
            .take(owned.len())
            .enumerate()
            .map(|(i, &key)| EventOption::with_arg(key, "TRADE", i))
            .collect();
        if options.is_empty() {
            options.push(EventOption::new("PROCEED", "DONE"));
        }
        *ctx.data = EventData::RelicTrader { owned, new };
        Ok(options)
    }

    /// `Trade(index)`: `RelicCmd.Remove` of the owned relic, then `RelicCmd.Obtain` of the new.
    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if action.tag == "TRADE" {
            let EventData::RelicTrader { owned, new } = &*ctx.data else {
                unreachable!("RelicTrader's relics are set in begin");
            };
            let (old, new) = (owned[action.arg], new[action.arg]);
            ctx.remove_relic(old);
            ctx.obtain_relic(new)?;
        }
        Ok(Page::finished("RELIC_TRADER", "DONE"))
    }
}
