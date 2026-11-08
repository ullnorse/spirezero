//! `WarHistorianRepy` (`Models.Events/WarHistorianRepy.cs`): never pulled on its own
//! (`IsAllowed` is false); a deck `LanternKey` turns every Act 3 `?` into it
//! (`LanternKey.ModifyNextEvent`). Each unlock spends a key: the cage gives `HistoryCourse`, the
//! chest 2 potion and 2 relic rewards. A player with a second key gets the other unlock too.

use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::run::{PendingReward, RunError, RunState};

/// `LanternKey`'s `ModelId.Entry`.
const LANTERN_KEY: &str = "LANTERN_KEY";
/// `HistoryCourse`'s `ModelId.Entry`.
const HISTORY_COURSE: &str = "HISTORY_COURSE";

const UNLOCK_CAGE: &str = "WAR_HISTORIAN_REPY.pages.INITIAL.options.UNLOCK_CAGE";
const UNLOCK_CHEST: &str = "WAR_HISTORIAN_REPY.pages.INITIAL.options.UNLOCK_CHEST";

pub struct WarHistorianRepy;

/// `ShouldGetSecondReward`: a `LanternKey` is still in the deck (single player).
fn should_get_second_reward(s: &RunState) -> bool {
    s.deck.iter().any(|c| c.id == LANTERN_KEY)
}

/// `RemoveFirstLanternKey` (`RemoveLanternKeysForInitialChoice` in single player):
/// `PlayerCmd.CompleteQuest` (a stat, not modeled), then `CardPileCmd.RemoveFromDeck`.
fn remove_first_key(s: &mut RunState) {
    if let Some(i) = s.deck.iter().position(|c| c.id == LANTERN_KEY) {
        s.remove_card(i);
    }
}

/// `RemoveLanternKeysForSecondChoice`: every `LanternKey`, in deck order.
fn remove_every_key(s: &mut RunState) {
    while let Some(i) = s.deck.iter().position(|c| c.id == LANTERN_KEY) {
        s.remove_card(i);
    }
}

/// `UnlockCage`: `ExtraFields.FreedRepy` (only read by a background node), then
/// `RelicCmd.Obtain<HistoryCourse>`.
fn unlock_cage(ctx: &mut EventCtx<'_>) -> Result<(), RunError> {
    ctx.obtain_relic(HISTORY_COURSE)
}

/// `UnlockChest`: `RewardsCmd.OfferCustom` of 2 `PotionReward`s and 2 `RelicReward`s.
fn unlock_chest(ctx: &mut EventCtx<'_>) {
    ctx.offer_pending_rewards(vec![
        PendingReward::Potion,
        PendingReward::Potion,
        PendingReward::Relic,
        PendingReward::Relic,
    ]);
}

impl EventModel for WarHistorianRepy {
    fn is_allowed(&self, _s: &RunState) -> bool {
        false
    }

    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(vec![
            EventOption::new(UNLOCK_CAGE, "UNLOCK_CAGE"),
            EventOption::new(UNLOCK_CHEST, "UNLOCK_CHEST"),
        ])
    }

    /// The initial unlocks spend the first key and then offer the other unlock while a key is
    /// left (read after the unlock's prompts); the second ones finish first, then spend every key.
    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        match action.tag {
            "UNLOCK_CAGE" => {
                remove_first_key(ctx.state());
                unlock_cage(ctx)?;
            }
            "UNLOCK_CHEST" => {
                remove_first_key(ctx.state());
                unlock_chest(ctx);
            }
            "SECOND_UNLOCK_CAGE" => {
                remove_every_key(ctx.state());
                unlock_cage(ctx)?;
                return Ok(Page::finished("WAR_HISTORIAN_REPY", "EXTRA_UNLOCK_CAGE"));
            }
            _ => {
                remove_every_key(ctx.state());
                unlock_chest(ctx);
                return Ok(Page::finished("WAR_HISTORIAN_REPY", "EXTRA_UNLOCK_CHEST"));
            }
        }
        ctx.then(action);
        Ok(Page::Next(Vec::new()))
    }

    fn after_prompt(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Page {
        if !should_get_second_reward(ctx.state()) {
            return Page::finished("WAR_HISTORIAN_REPY", action.tag);
        }
        Page::Next(vec![if action.tag == "UNLOCK_CAGE" {
            EventOption::new(UNLOCK_CHEST, "SECOND_UNLOCK_CHEST")
        } else {
            EventOption::new(UNLOCK_CAGE, "SECOND_UNLOCK_CAGE")
        }])
    }
}
