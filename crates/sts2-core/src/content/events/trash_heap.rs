//! `TrashHeap` (`Models.Events/TrashHeap.cs`): lose 8 HP for a random relic of five, or take 100
//! gold and a random card of ten.

use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::decimal::Decimal;
use crate::run::{RunCard, RunError, RunState};

/// `HpLossVar(8m)`.
const HP_LOSS: i32 = 8;
/// `GoldVar(100)`.
const GOLD: i32 = 100;
/// `IsAllowed`: the player needs more HP than this.
const MIN_HP: i32 = 5;
/// `Relics`.
const RELICS: [&str; 5] = [
    "DARKSTONE_PERIAPT",
    "DREAM_CATCHER",
    "HAND_DRILL",
    "MAW_BANK",
    "THE_BOOT",
];
/// `Cards`.
const CARDS: [&str; 10] = [
    "CALTROPS",
    "CLASH",
    "DISTRACTION",
    "DUAL_WIELD",
    "ENTRENCH",
    "HELLO_WORLD",
    "OUTMANEUVER",
    "REBOUND",
    "RIP_AND_TEAR",
    "STACK",
];

pub struct TrashHeap;

impl EventModel for TrashHeap {
    fn is_allowed(&self, s: &RunState) -> bool {
        s.hp > MIN_HP
    }

    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(vec![
            EventOption::new("TRASH_HEAP.pages.INITIAL.options.DIVE_IN", "DIVE_IN"),
            EventOption::new("TRASH_HEAP.pages.INITIAL.options.GRAB", "GRAB"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if action.tag == "DIVE_IN" {
            // `DiveIn`: the unblockable damage, then a relic on the event's `Rng`.
            ctx.state().damage(HP_LOSS);
            let relic = *ctx.rng.next_item(&RELICS).expect("five relics");
            ctx.obtain_relic(relic)?;
        } else {
            // `Grab`: the gold, then a card on the event's `Rng`.
            ctx.state().gain_gold(Decimal::from(GOLD));
            let card = *ctx.rng.next_item(&CARDS).expect("ten cards");
            ctx.state().add_card(RunCard::new(card));
        }
        Ok(Page::finished("TRASH_HEAP", action.tag))
    }
}
