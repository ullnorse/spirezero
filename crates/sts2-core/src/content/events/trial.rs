//! `Trial` (`Models.Events/Trial.cs`): accept a verdict to deliver, or reject. A random case
//! (merchant, noble or nondescript, `Rng.NextInt(3)`) asks for guilty or innocent, each with its
//! own reward and curse. Rejecting leads to accepting anyway or doubling down, which opens the
//! abandon-run popup.

use crate::combat::PromptText;
use crate::content::events::{EventAction, EventCtx, EventData, EventModel, EventOption, Page};
use crate::decimal::Decimal;
use crate::run::{CardCreation, CardPool, RunCard, RunError, SelectPurpose};

/// `new CardReward(.., 3, Owner)` twice.
const REWARD_COUNT: usize = 2;
const REWARD_CARDS: usize = 3;
/// `PullNextRelicFromFront` twice for the guilty merchant.
const MERCHANT_RELICS: usize = 2;
/// `CardSelectorPrefs(.., 2)` for the upgrades and transforms.
const PICKS: usize = 2;
/// `Heal(Owner.Creature, 10m)`.
const NOBLE_HEAL: i32 = 10;
/// `GainGold(300m)`.
const NOBLE_GOLD: i32 = 300;

pub struct Trial;

fn option(key: &str, tag: &'static str) -> EventOption {
    EventOption::new(format!("TRIAL.pages.{key}"), tag)
}

/// The initial page's options (`Reject`'s "accept" goes to the same case roll).
/// The case pages' `TrialStory`: the case's description (`TRIAL.pages.MERCHANT.description`).
pub(super) fn story_key(case: &str) -> Option<&'static str> {
    Some(match case {
        "MERCHANT" => "TRIAL.pages.MERCHANT.description",
        "NOBLE" => "TRIAL.pages.NOBLE.description",
        "NONDESCRIPT" => "TRIAL.pages.NONDESCRIPT.description",
        _ => return None,
    })
}

/// `SetTrialFinished(trialResultLoc)`'s `TrialResult`: the verdict's page description.
pub(super) fn result_key(verdict: &str) -> Option<&'static str> {
    Some(match verdict {
        "MERCHANT_GUILTY" => "TRIAL.pages.MERCHANT_GUILTY.description",
        "MERCHANT_INNOCENT" => "TRIAL.pages.MERCHANT_INNOCENT.description",
        "NOBLE_GUILTY" => "TRIAL.pages.NOBLE_GUILTY.description",
        "NOBLE_INNOCENT" => "TRIAL.pages.NOBLE_INNOCENT.description",
        "NONDESCRIPT_GUILTY" => "TRIAL.pages.NONDESCRIPT_GUILTY.description",
        "NONDESCRIPT_INNOCENT" => "TRIAL.pages.NONDESCRIPT_INNOCENT.description",
        _ => return None,
    })
}

/// `Accept`: a case drawn on the event's `Rng` (kept for its page's `TrialStory`), and its two
/// verdicts.
fn case_options(ctx: &mut EventCtx<'_>) -> Vec<EventOption> {
    let case = match ctx.rng.next_int(3) {
        0 => "MERCHANT",
        1 => "NOBLE",
        _ => "NONDESCRIPT",
    };
    *ctx.data = EventData::Trial {
        case: Some(case),
        verdict: None,
    };
    match case {
        "MERCHANT" => vec![
            option("MERCHANT.options.GUILTY", "MERCHANT_GUILTY"),
            option("MERCHANT.options.INNOCENT", "MERCHANT_INNOCENT"),
        ],
        "NOBLE" => vec![
            option("NOBLE.options.GUILTY", "NOBLE_GUILTY"),
            option("NOBLE.options.INNOCENT", "NOBLE_INNOCENT"),
        ],
        _ => vec![
            option("NONDESCRIPT.options.GUILTY", "NONDESCRIPT_GUILTY"),
            option("NONDESCRIPT.options.INNOCENT", "NONDESCRIPT_INNOCENT"),
        ],
    }
}

fn reject_options() -> Vec<EventOption> {
    vec![
        option("REJECT.options.ACCEPT", "ACCEPT"),
        option("REJECT.options.DOUBLE_DOWN", "DOUBLE_DOWN"),
    ]
}

impl EventModel for Trial {
    /// `CalculateVars` rolls `EntrantNumber` on `Rng.Chaotic` (text only).
    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(vec![
            option("INITIAL.options.ACCEPT", "ACCEPT"),
            option("INITIAL.options.REJECT", "REJECT"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        if let EventData::Trial { case, .. } = *ctx.data
            && result_key(action.tag).is_some()
        {
            *ctx.data = EventData::Trial {
                case,
                verdict: Some(action.tag),
            };
        }
        match action.tag {
            "ACCEPT" => return Ok(Page::Next(case_options(ctx))),
            // `DoubleDown` opens `NAbandonRunConfirmPopup`: its option stays
            // (`disableOnChosen` false).
            "REJECT" | "DOUBLE_DOWN" => return Ok(Page::Next(reject_options())),
            "MERCHANT_GUILTY" => {
                ctx.state().add_card(RunCard::new("REGRET"));
                for _ in 0..MERCHANT_RELICS {
                    let relic = ctx.pull_relic_from_front();
                    ctx.obtain_relic(relic)?;
                }
            }
            "MERCHANT_INNOCENT" => {
                ctx.state().add_card(RunCard::new("SHAME"));
                let options = ctx.state().upgradable_cards();
                ctx.select(SelectPurpose::Upgrade, options, PICKS)?;
            }
            "NOBLE_GUILTY" => ctx.state().heal(Decimal::from(NOBLE_HEAL)),
            "NOBLE_INNOCENT" => {
                let s = ctx.state();
                s.add_card(RunCard::new("REGRET"));
                s.gain_gold(Decimal::from(NOBLE_GOLD));
            }
            "NONDESCRIPT_GUILTY" => {
                ctx.state().add_card(RunCard::new("DOUBT"));
                let options =
                    CardCreation::non_combat_default(CardPool::Character).as_card_reward();
                let rewards = (0..REWARD_COUNT)
                    .map(|_| ctx.card_reward(REWARD_CARDS, options))
                    .collect();
                ctx.offer_rewards(rewards);
            }
            _ => {
                ctx.state().add_card(RunCard::new("DOUBT"));
                let options = ctx.state().transformable_cards();
                ctx.select_deck_cards(action, PromptText::Transform, options, PICKS)?;
            }
        }
        Ok(Page::Finished("TRIAL.trialResult".to_owned()))
    }

    /// `NondescriptInnocent` after `FromDeckForTransformation`: `TransformToRandom` on the event's
    /// `Rng`, per picked card in pick order.
    fn on_cards_selected(
        &self,
        _action: EventAction,
        ctx: &mut EventCtx<'_>,
        cards: &[usize],
    ) -> Result<(), RunError> {
        for (n, &i) in cards.iter().enumerate() {
            // Each transformed card left its place and its replacement joined the end.
            let shift = cards[..n].iter().filter(|&&j| j < i).count();
            ctx.transform_to_random(i - shift)?;
        }
        Ok(())
    }
}
