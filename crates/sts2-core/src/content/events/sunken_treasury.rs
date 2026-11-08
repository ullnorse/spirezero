//! `SunkenTreasury` (`Models.Events/SunkenTreasury.cs`): about 60 gold, or about 333 gold and a
//! `Greed`.

use crate::content::events::{EventAction, EventCtx, EventData, EventModel, EventOption, Page};
use crate::decimal::Decimal;
use crate::run::{RunCard, RunError};

/// `DynamicVar("SmallChestGold", 60m)`.
const SMALL_GOLD: i32 = 60;
/// `DynamicVar("LargeChestGold", 333m)`.
const LARGE_GOLD: i32 = 333;

pub struct SunkenTreasury;

impl EventModel for SunkenTreasury {
    /// `CalculateVars`: `SmallChestGold += NextInt(16) - 8`, then `LargeChestGold +=
    /// NextInt(61) - 30`.
    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let small = SMALL_GOLD + ctx.rng.next_int(16) - 8;
        let large = LARGE_GOLD + ctx.rng.next_int(61) - 30;
        *ctx.data = EventData::SunkenTreasury {
            small: Decimal::from(small),
            large: Decimal::from(large),
        };
        Ok(vec![
            EventOption::new(
                "SUNKEN_TREASURY.pages.INITIAL.options.FIRST_CHEST",
                "FIRST_CHEST",
            ),
            EventOption::new(
                "SUNKEN_TREASURY.pages.INITIAL.options.SECOND_CHEST",
                "SECOND_CHEST",
            ),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        let EventData::SunkenTreasury { small, large } = *ctx.data else {
            unreachable!("SunkenTreasury's gold is set in begin");
        };
        if action.tag == "FIRST_CHEST" {
            ctx.state().gain_gold(small);
        } else {
            let s = ctx.state();
            s.gain_gold(large);
            s.add_card(RunCard::new("GREED"));
        }
        Ok(Page::finished("SUNKEN_TREASURY", action.tag))
    }
}
