//! Event and ancient choices by what the screen shows.
//!
//! An event option's text key and an
//! ancient option's relic id, looked up in [`table::OPTION_VALUES`], the mean change in
//! [`run_value`](super::run::run_value) the option was measured to bring on the **train** seeds
//! (`sts2-diff bot --measure-options`, which applies each option on a copy of a train run and
//! plays the bot on to the next map screen). At play time nothing is applied: the table is a
//! player's experience of the event, not a look at this run's roll. An option the table does
//! not know is judged by its words: leaving is safe, anything else is a small risk.

use sts2_core::content::ancients::AncientRoom;
use sts2_core::content::events::EventVar;
use sts2_core::run::events::EventRoom;
use sts2_core::run::{RunAction, RunState};

use super::run::relic_bonus;
use super::table;

/// The fewest samples before a measured value is trusted.
const MIN_SAMPLES: u32 = 3;

/// The table key of an ancient's relic option.
#[must_use]
pub fn ancient_key(relic: &str) -> String {
    format!("ANCIENT:{relic}")
}

/// A measured value and its sample count.
fn measured(key: &str) -> Option<i64> {
    table::OPTION_VALUES
        .binary_search_by(|(k, ..)| (*k).cmp(key))
        .ok()
        .map(|i| table::OPTION_VALUES[i])
        .filter(|(_, _, n)| *n >= MIN_SAMPLES)
        .map(|(_, v, _)| i64::from(v))
}

/// Whether an option's text key reads as walking away.
fn is_leave(key: &str) -> bool {
    let last = key.rsplit('.').next().unwrap_or(key);
    [
        "LEAVE",
        "REFUSE",
        "IGNORE",
        "SKIP",
        "DECLINE",
        "WALK_AWAY",
        "PROCEED",
        "RUN",
        "FLEE",
        "NOTHING",
        "CONTINUE",
        "BACK_AWAY",
        "EXIT",
    ]
    .iter()
    .any(|w| last.contains(w))
}

/// The value of an event option with text key `key`.
fn event_option_value(key: &str) -> i64 {
    measured(key).unwrap_or_else(|| if is_leave(key) { 0 } else { -5 })
}

/// The cost charged to a risky option while the event shows damage that would leave the player
/// below [`SAFE_HP_PERCENT`] of max HP.
const DANGER: i64 = 1_000;
/// The share of max HP (percent) an event's shown damage may not take the player below.
const SAFE_HP_PERCENT: i32 = 30;

/// Whether the event's text shows an HP loss that would leave the player too low: the page's
/// displayed `Damage` / `...HpLoss` vars against the current HP.
fn dangerous(s: &RunState, room: &EventRoom) -> bool {
    room.vars(s).iter().any(|(name, var)| {
        let EventVar::Number(amount) = var else {
            return false;
        };
        (name.ends_with("Damage") || name.ends_with("HpLoss") && !name.contains("MaxHp"))
            && s.hp - amount.to_int_truncating() < s.max_hp * SAFE_HP_PERCENT / 100
    })
}

/// The event option to take: the best measured value among the legal ones (the first on ties).
/// While the page shows a dangerous HP loss, every option but walking away is charged for it.
#[must_use]
pub fn event_choice(s: &RunState, room: &EventRoom, legal: &[RunAction]) -> Option<RunAction> {
    let danger = dangerous(s, room);
    room.options
        .iter()
        .enumerate()
        .filter(|(i, _)| legal.contains(&RunAction::EventOption(*i)))
        .map(|(i, o)| {
            let risk = if danger && !is_leave(&o.text_key) {
                DANGER
            } else {
                0
            };
            (event_option_value(&o.text_key) - risk, i)
        })
        .rev()
        .max_by_key(|(v, _)| *v)
        .map(|(_, i)| RunAction::EventOption(i))
}

/// The ancient option to take: the best measured relic (its generic value if unmeasured).
#[must_use]
pub fn ancient_choice(_s: &RunState, room: &AncientRoom, legal: &[RunAction]) -> Option<RunAction> {
    room.options
        .iter()
        .enumerate()
        .filter(|(i, _)| legal.contains(&RunAction::AncientOption(*i)))
        .map(|(i, o)| {
            let value = o.relic_id().map_or(-1_000, |id| {
                measured(&ancient_key(id)).unwrap_or_else(|| 30 + relic_bonus(id))
            });
            (value, i)
        })
        .rev()
        .max_by_key(|(v, _)| *v)
        .map(|(_, i)| RunAction::AncientOption(i))
}
