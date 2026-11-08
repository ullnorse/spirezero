//! `ColossalFlower` (`Models.Events/ColossalFlower.cs`): take the current prize (35, 75 or 135
//! gold) or reach deeper for 5, then 6 damage; after two digs, 135 gold or 7 more damage for a
//! `PollinousCore`.

use crate::content::events::{EventAction, EventCtx, EventData, EventModel, EventOption, Page};
use crate::decimal::Decimal;
use crate::run::{RunError, RunState};

/// `_prizeCosts`: the gold of each prize.
pub(super) const PRIZE_COSTS: [i32; 3] = [35, 75, 135];
/// `_prizeDamage`: the damage of each dig.
const PRIZE_DAMAGE: [i32; 3] = [5, 6, 7];
/// `IsAllowed`: `CurrentHp >= 19`.
const MIN_HP: i32 = 19;
/// The page after this many digs offers the relic.
const MAX_DIGS: usize = 2;

pub struct ColossalFlower;

/// `NumberOfDigs`.
fn digs(data: &EventData) -> usize {
    match *data {
        EventData::None => 0,
        EventData::ColossalFlower { digs } => digs,
        _ => unreachable!("ColossalFlower keeps its digs"),
    }
}

/// The two options of a page: extract or reach deeper, keyed by the page and dig count.
fn dig_options(page: &str, digs: usize) -> Vec<EventOption> {
    let next = digs + 1;
    vec![
        EventOption::new(
            format!("COLOSSAL_FLOWER.pages.{page}.options.EXTRACT_CURRENT_PRIZE_{next}"),
            "EXTRACT_CURRENT_PRIZE",
        ),
        EventOption::new(
            format!("COLOSSAL_FLOWER.pages.{page}.options.REACH_DEEPER_{next}"),
            "REACH_DEEPER",
        ),
    ]
}

impl EventModel for ColossalFlower {
    fn is_allowed(&self, s: &RunState) -> bool {
        s.hp >= MIN_HP
    }

    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        *ctx.data = EventData::ColossalFlower { digs: 0 };
        Ok(dig_options("INITIAL", 0))
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        let digs = digs(ctx.data);
        match action.tag {
            "REACH_DEEPER" => {
                // `DealReachDeeperDamage`: unblockable, unpowered.
                ctx.state().damage(PRIZE_DAMAGE[digs]);
                let digs = digs + 1;
                *ctx.data = EventData::ColossalFlower { digs };
                if digs < MAX_DIGS {
                    return Ok(Page::Next(dig_options(
                        &format!("REACH_DEEPER_{digs}"),
                        digs,
                    )));
                }
                Ok(Page::Next(vec![
                    EventOption::new(
                        "COLOSSAL_FLOWER.pages.REACH_DEEPER_2.options.EXTRACT_INSTEAD",
                        "EXTRACT_INSTEAD",
                    ),
                    EventOption::new(
                        "COLOSSAL_FLOWER.pages.REACH_DEEPER_2.options.POLLINOUS_CORE",
                        "POLLINOUS_CORE",
                    ),
                ]))
            }
            "POLLINOUS_CORE" => {
                ctx.state().damage(PRIZE_DAMAGE[digs]);
                ctx.obtain_relic("POLLINOUS_CORE")?;
                Ok(Page::finished("COLOSSAL_FLOWER", "POLLINOUS_CORE"))
            }
            // `ExtractCurrentPrize`, `ExtractInstead`.
            _ => {
                ctx.state().gain_gold(Decimal::from(PRIZE_COSTS[digs]));
                Ok(Page::finished("COLOSSAL_FLOWER", action.tag))
            }
        }
    }
}
