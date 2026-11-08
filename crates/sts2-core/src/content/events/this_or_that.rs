//! `ThisOrThat` (`Models.Events/ThisOrThat.cs`): 6 damage for 41-68 gold, or a relic from the
//! bag's front with a `Clumsy`.

use crate::content::events::{EventAction, EventCtx, EventData, EventModel, EventOption, Page};
use crate::decimal::Decimal;
use crate::run::{RunCard, RunError};

/// `HpLossVar(6m)`.
const HP_LOSS: i32 = 6;

pub struct ThisOrThat;

impl EventModel for ThisOrThat {
    /// `CalculateVars`: `Gold = Rng.NextInt(41, 69)`.
    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let gold = ctx.rng.next_int_range(41, 69);
        *ctx.data = EventData::Amount(Decimal::from(gold));
        Ok(vec![
            EventOption::new("THIS_OR_THAT.pages.INITIAL.options.PLAIN", "PLAIN"),
            EventOption::new("THIS_OR_THAT.pages.INITIAL.options.ORNATE", "ORNATE"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if action.tag == "PLAIN" {
            let EventData::Amount(gold) = *ctx.data else {
                unreachable!("ThisOrThat's gold is set in begin");
            };
            ctx.state().damage(HP_LOSS);
            ctx.state().gain_gold(gold);
        } else {
            // `Ornate`.
            let relic = ctx.pull_relic_from_front();
            ctx.obtain_relic(relic)?;
            ctx.state().add_card(RunCard::new("CLUMSY"));
        }
        Ok(Page::finished("THIS_OR_THAT", action.tag))
    }
}
