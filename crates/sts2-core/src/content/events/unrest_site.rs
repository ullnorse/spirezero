//! `UnrestSite` (`Models.Events/UnrestSite.cs`): heal to full with a `PoorSleep`, or lose 8 max
//! HP for a relic from the bag's front.

use crate::content::events::{EventAction, EventCtx, EventData, EventModel, EventOption, Page};
use crate::decimal::Decimal;
use crate::run::{RunCard, RunError, RunState};

/// `DynamicVar("MaxHpLoss", 8m)`.
const MAX_HP_LOSS: i32 = 8;

pub struct UnrestSite;

impl EventModel for UnrestSite {
    /// `CurrentHp <= MaxHp * 0.70m`.
    fn is_allowed(&self, s: &RunState) -> bool {
        Decimal::from(s.hp) <= Decimal::from(s.max_hp) * Decimal::new(70, 2)
    }

    /// `CalculateVars`: `Heal = MaxHp - CurrentHp`.
    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let s = ctx.state();
        let heal = s.max_hp - s.hp;
        *ctx.data = EventData::Amount(Decimal::from(heal));
        Ok(vec![
            EventOption::new("UNREST_SITE.pages.INITIAL.options.REST", "REST"),
            EventOption::new("UNREST_SITE.pages.INITIAL.options.KILL", "KILL"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if action.tag == "REST" {
            let EventData::Amount(heal) = *ctx.data else {
                unreachable!("UnrestSite's heal is set in begin");
            };
            ctx.state().heal(heal);
            ctx.state().add_card(RunCard::new("POOR_SLEEP"));
        } else {
            // `Kill`.
            ctx.state().lose_max_hp(MAX_HP_LOSS);
            let relic = ctx.pull_relic_from_front();
            ctx.obtain_relic(relic)?;
        }
        Ok(Page::finished("UNREST_SITE", action.tag))
    }
}
