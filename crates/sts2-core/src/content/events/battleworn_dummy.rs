//! `BattlewornDummy` (`Models.Events/BattlewornDummy.cs`): fight one of three training dummies
//! (`BattlewornDummyEventV1..V3Encounter`, no room rewards) and come back to the event
//! (`Resume`). Beating the dummy before its `BattlewornDummyTimeLimitPower` runs out gives a
//! potion reward (V1: `NextItem` on `Rewards` over the character's and the shared pool),
//! 2 upgrades (V2: `StableShuffle` of the upgradable deck cards on the event `Rng`) or a relic
//! reward (V3: `PullNextRelicFromFront`); the fight's `ExtraRewards` follow.

use crate::combat::Encounter;
use crate::content::encounters::EncounterData;
use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, ExitedCombat, Page};
use crate::rng::PlayerRngType;
use crate::run::potions::potion_options;
use crate::run::{PendingReward, Reward, RunError};
use crate::shuffle::stable_shuffle_by;

/// `Take(2)` of the shuffled upgradable cards (V2).
const UPGRADES: usize = 2;

pub struct BattlewornDummy;

/// `GenerateInitialOptions`'s options (the `SettingNHp` vars are text only).
fn initial_options() -> Vec<EventOption> {
    vec![
        EventOption::new(
            "BATTLEWORN_DUMMY.pages.INITIAL.options.SETTING_1",
            "SETTING_1",
        ),
        EventOption::new(
            "BATTLEWORN_DUMMY.pages.INITIAL.options.SETTING_2",
            "SETTING_2",
        ),
        EventOption::new(
            "BATTLEWORN_DUMMY.pages.INITIAL.options.SETTING_3",
            "SETTING_3",
        ),
    ]
}

impl EventModel for BattlewornDummy {
    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(initial_options())
    }

    /// `EnterCombatWithoutExitingEvent(encounter, [], shouldResumeAfterCombat: true)`; the page
    /// stays until `Resume`.
    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        let encounter = match action.tag {
            "SETTING_1" => Encounter::BattlewornDummyEventV1Encounter,
            "SETTING_2" => Encounter::BattlewornDummyEventV2Encounter,
            _ => Encounter::BattlewornDummyEventV3Encounter,
        };
        ctx.enter_combat(encounter.entry(), Vec::new(), true)?;
        Ok(Page::Next(initial_options()))
    }

    fn resume(&self, exited: &ExitedCombat, ctx: &mut EventCtx<'_>) -> Option<Page> {
        if exited.data
            == (EncounterData::BattlewornDummy {
                ran_out_of_time: true,
            })
        {
            return Some(Page::finished("BATTLEWORN_DUMMY", "DEFEAT"));
        }
        let mut rewards = Vec::new();
        match exited.encounter {
            Encounter::BattlewornDummyEventV1Encounter => {
                let s = ctx.state();
                let potions: Vec<&str> = potion_options(s.character)
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect();
                if let Some(&potion) = s.player_rng[PlayerRngType::Rewards].next_item(&potions) {
                    rewards.push(PendingReward::Ready(Reward::Potion(potion)));
                }
            }
            Encounter::BattlewornDummyEventV2Encounter => {
                // `Deck.Cards.Where(IsUpgradable).ToList()`, in deck order.
                let mut cards: Vec<usize> = (0..ctx.state().deck.len())
                    .filter(|&i| ctx.state().deck[i].is_upgradable())
                    .collect();
                let deck = ctx.state().deck.clone();
                stable_shuffle_by(&mut cards, ctx.rng, |&a, &b| deck[a].compare_to(&deck[b]));
                for &i in cards.iter().take(UPGRADES) {
                    ctx.state().deck[i].upgrade();
                }
            }
            _ => {
                let relic = ctx.pull_relic_from_front();
                rewards.push(PendingReward::Ready(Reward::Relic(relic)));
            }
        }
        rewards.extend(exited.extra_rewards.iter().cloned());
        if !rewards.is_empty() {
            ctx.offer_pending_rewards(rewards);
        }
        Some(Page::finished("BATTLEWORN_DUMMY", "VICTORY"))
    }
}
