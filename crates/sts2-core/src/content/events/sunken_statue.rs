//! `SunkenStatue` (`Models.Events/SunkenStatue.cs`): take `SwordOfStone`, or dive for about 111
//! gold and 7 damage.

use crate::content::events::{EventAction, EventCtx, EventData, EventModel, EventOption, Page};
use crate::decimal::Decimal;
use crate::run::RunError;

/// `DynamicVar("HpLoss", 7m)`.
const HP_LOSS: i32 = 7;

pub struct SunkenStatue;

impl EventModel for SunkenStatue {
    /// `CalculateVars`: `Gold 111 += Rng.NextInt(-10, 11)`.
    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let gold = 111 + ctx.rng.next_int_range(-10, 11);
        *ctx.data = EventData::Amount(Decimal::from(gold));
        Ok(vec![
            EventOption::new(
                "SUNKEN_STATUE.pages.INITIAL.options.GRAB_SWORD",
                "GRAB_SWORD",
            ),
            EventOption::new(
                "SUNKEN_STATUE.pages.INITIAL.options.DIVE_INTO_WATER",
                "DIVE_INTO_WATER",
            ),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if action.tag == "GRAB_SWORD" {
            ctx.obtain_relic("SWORD_OF_STONE")?;
        } else {
            let EventData::Amount(gold) = *ctx.data else {
                unreachable!("SunkenStatue's gold is set in begin");
            };
            ctx.state().gain_gold(gold);
            ctx.state().damage(HP_LOSS);
        }
        Ok(Page::finished("SUNKEN_STATUE", action.tag))
    }
}
