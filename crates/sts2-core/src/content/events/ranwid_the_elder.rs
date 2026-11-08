//! `RanwidTheElder` (`Models.Events/RanwidTheElder.cs`): trade a random potion or 100 gold for a
//! relic, or a random tradable relic for two. Shared, Act 2 and 3; the belt is locked while it
//! runs (`CanUseOrRemovePotions`).

use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::run::relics::is_tradable;
use crate::run::{RunError, RunState};

/// `GoldVar(100)`.
const GOLD: i32 = 100;
/// `GiveRelic` obtains two relics.
const RELICS_FOR_A_RELIC: usize = 2;

pub struct RanwidTheElder;

/// The indices of the owned relics that are `IsTradable`, in relic order.
fn tradable_relics(s: &RunState) -> Vec<usize> {
    (0..s.relics.len())
        .filter(|&i| is_tradable(&s.relics[i]))
        .collect()
}

impl EventModel for RanwidTheElder {
    /// Not Act 1, a tradable relic, 100 gold and a potion (`RanwidTheElder.cs:51-70`).
    fn is_allowed(&self, s: &RunState) -> bool {
        s.act_index != 0
            && !tradable_relics(s).is_empty()
            && s.gold >= GOLD
            && s.potions_held().next().is_some()
    }

    fn locks_potions(&self) -> bool {
        true
    }

    /// `Rng.NextItem(Owner.Potions)` (its belt slot is the potion option's argument), then
    /// `Rng.NextItem` of the tradable relics (its relic index is the relic option's).
    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let slots: Vec<usize> = ctx.state().potions_held().map(|(slot, _)| slot).collect();
        let potion = match ctx.rng.next_item(&slots) {
            Some(&slot) => EventOption::with_arg(
                "RANWID_THE_ELDER.pages.INITIAL.options.POTION",
                "POTION",
                slot,
            ),
            None => EventOption::locked("RANWID_THE_ELDER.pages.INITIAL.options.POTION_LOCKED"),
        };
        let relics = tradable_relics(ctx.state());
        let relic = match ctx.rng.next_item(&relics) {
            Some(&i) => {
                EventOption::with_arg("RANWID_THE_ELDER.pages.INITIAL.options.RELIC", "RELIC", i)
            }
            None => EventOption::locked("RANWID_THE_ELDER.pages.INITIAL.options.RELIC_LOCKED"),
        };
        Ok(vec![
            potion,
            EventOption::new("RANWID_THE_ELDER.pages.INITIAL.options.GOLD", "GOLD"),
            relic,
        ])
    }

    /// Each relic is `RelicFactory.PullNextRelicFromFront(Owner)` then `RelicCmd.Obtain`.
    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        let count = match action.tag {
            "POTION" => {
                ctx.state().discard_potion(action.arg);
                1
            }
            "GOLD" => {
                ctx.state().lose_gold(GOLD);
                1
            }
            _ => {
                ctx.remove_relic(action.arg);
                RELICS_FOR_A_RELIC
            }
        };
        for _ in 0..count {
            let relic = ctx.pull_relic_from_front();
            ctx.obtain_relic(relic)?;
        }
        Ok(Page::finished("RANWID_THE_ELDER", action.tag))
    }
}
