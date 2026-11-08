//! `TabletOfTruth` (`Models.Events/TabletOfTruth.cs`): decipher up to five times, each losing
//! more max HP for a random upgrade (all upgrades the fifth time), or smash it to heal 20.

use crate::content::events::{EventAction, EventCtx, EventData, EventModel, EventOption, Page};
use crate::decimal::Decimal;
use crate::run::RunError;

/// `DynamicVar("SmashHPGain", 20m)`.
const SMASH_HEAL: i32 = 20;
/// `DynamicVar("DecipherMaxHpLoss", 3m)`.
const FIRST_COST: i32 = 3;

pub struct TabletOfTruth;

impl TabletOfTruth {
    /// `LoseMaxHpAndUpgrade(hp)`: a cost of all max HP kills; otherwise lose it, then upgrade
    /// a random upgradable card on the event `Rng` (every one on the fifth decipher).
    fn lose_max_hp_and_upgrade(ctx: &mut EventCtx<'_>, cost: i32, decipher_count: i32) {
        let s = ctx.state();
        if cost >= s.max_hp {
            let all_but_one = s.max_hp - 1;
            s.lose_max_hp(all_but_one);
            s.kill();
            return;
        }
        s.lose_max_hp(cost);
        let upgradable = s.upgradable_cards();
        if decipher_count == 4 {
            for i in upgradable {
                ctx.state().deck[i].upgrade();
            }
        } else if let Some(&i) = ctx.rng.next_item(&upgradable) {
            ctx.state().deck[i].upgrade();
        }
    }

    /// `GetDecipherCost()` after `count` deciphers.
    fn decipher_cost(ctx: &mut EventCtx<'_>, count: i32) -> i32 {
        match count {
            1 => 6,
            2 => 12,
            3 => 24,
            _ => ctx.state().max_hp - 1,
        }
    }
}

impl EventModel for TabletOfTruth {
    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        *ctx.data = EventData::TabletOfTruth {
            decipher_count: 0,
            cost: FIRST_COST,
        };
        Ok(vec![
            EventOption::new(
                "TABLET_OF_TRUTH.pages.INITIAL.options.DECIPHER_1",
                "DECIPHER",
            ),
            EventOption::new("TABLET_OF_TRUTH.pages.INITIAL.options.SMASH", "SMASH"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        let EventData::TabletOfTruth {
            decipher_count,
            cost,
        } = *ctx.data
        else {
            unreachable!("TabletOfTruth's vars are set in begin");
        };
        match action.tag {
            "SMASH" => {
                ctx.state().heal(Decimal::from(SMASH_HEAL));
                Ok(Page::finished("TABLET_OF_TRUTH", "SMASH"))
            }
            "GIVE_UP" => Ok(Page::finished("TABLET_OF_TRUTH", "GIVE_UP")),
            _ => {
                Self::lose_max_hp_and_upgrade(ctx, cost, decipher_count);
                let count = decipher_count + 1;
                if count == 5 {
                    *ctx.data = EventData::TabletOfTruth {
                        decipher_count: count,
                        cost,
                    };
                    return Ok(Page::finished("TABLET_OF_TRUTH", "DECIPHER_5"));
                }
                let cost = Self::decipher_cost(ctx, count);
                *ctx.data = EventData::TabletOfTruth {
                    decipher_count: count,
                    cost,
                };
                Ok(Page::Next(vec![
                    EventOption::new(
                        format!("TABLET_OF_TRUTH.pages.DECIPHER_{count}.options.DECIPHER"),
                        "DECIPHER",
                    ),
                    EventOption::new("TABLET_OF_TRUTH.pages.DECIPHER.options.GIVE_UP", "GIVE_UP"),
                ]))
            }
        }
    }
}
