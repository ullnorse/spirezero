//! `DenseVegetation` (`Models.Events/DenseVegetation.cs`): trudge on for gold at 8 HP, or rest
//! (a mimicked rest-site heal) and then fight four `Wriggler`s.

use crate::content::events::{EventAction, EventCtx, EventData, EventModel, EventOption, Page};
use crate::decimal::Decimal;
use crate::run::RunError;

/// `HpLossVar(8m)`.
const HP_LOSS: i32 = 8;

pub struct DenseVegetation;

impl EventModel for DenseVegetation {
    /// `CalculateVars`: `Gold = Rng.NextInt(61, 100)` (`Heal` is display only). `IsAllowed` is
    /// `true` for a single player.
    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let gold = ctx.rng.next_int_range(61, 100);
        *ctx.data = EventData::Amount(Decimal::from(gold));
        Ok(vec![
            EventOption::new(
                "DENSE_VEGETATION.pages.INITIAL.options.TRUDGE_ON",
                "TRUDGE_ON",
            ),
            EventOption::new("DENSE_VEGETATION.pages.INITIAL.options.REST", "REST"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        match action.tag {
            // The VFX loop draws `Rng.Chaotic` only with a node tree.
            "TRUDGE_ON" => {
                let EventData::Amount(gold) = *ctx.data else {
                    unreachable!("DenseVegetation's gold is set in begin");
                };
                ctx.state().damage(HP_LOSS);
                ctx.state().gain_gold(gold);
                Ok(Page::finished("DENSE_VEGETATION", "TRUDGE_ON"))
            }
            "REST" => {
                let rewards = ctx.state().rest_site_heal(true);
                if !rewards.is_empty() {
                    ctx.offer_pending_rewards(rewards);
                }
                Ok(Page::Next(vec![EventOption::new(
                    "DENSE_VEGETATION.pages.REST.options.FIGHT",
                    "FIGHT",
                )]))
            }
            _ => {
                ctx.enter_combat("DENSE_VEGETATION_EVENT_ENCOUNTER", Vec::new(), false)?;
                Ok(Page::left())
            }
        }
    }
}
