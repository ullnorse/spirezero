//! The bot's advice for the decision on show (`w`).
//!
//! It is the honest bot the web's "ask the bot" uses (`sts2_diff::honest`, as `sts2-web-bot`
//! builds it), run on a thread of its own so the screen never waits for it.
//!
//! The bot reads only what the player sees (`docs/specs/honesty.md`). A request carries a copy
//! of the run; the answer is the bot's action and why, turned into words by [`words`] from the
//! app's own screen (the key to press and the row it is on).

use std::fmt;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread;

use sts2_core::Action;
use sts2_core::combat::Decision;
use sts2_core::run::{Reward, Run, RunAction, RunDecision};
use sts2_diff::honest::Bot;
use sts2_diff::honest::advice::{PHONE, Why};
use sts2_view::combat::POTION_KEYS;
use sts2_view::names;

use crate::app::App;

/// The bot's answer to a request: how many actions the run had when it was asked (the answer
/// is dropped if the run moved on), its action and why.
struct Answer {
    at: usize,
    advice: Option<(RunAction, Why)>,
}

/// The thread that holds the bot (built on the first request: its nets take a moment to parse)
/// and the one request in flight.
#[derive(Default)]
pub struct Hinter {
    worker: Option<Worker>,
    waiting: bool,
}

struct Worker {
    requests: Sender<(usize, Run)>,
    answers: Receiver<Answer>,
}

impl fmt::Debug for Hinter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Hinter")
            .field("waiting", &self.waiting)
            .finish_non_exhaustive()
    }
}

/// A copy of a shell has no bot running: it starts its own on the first request.
impl Clone for Hinter {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl Hinter {
    /// Whether an answer is on its way (the event loop wakes up to look for it).
    #[must_use]
    pub const fn thinking(&self) -> bool {
        self.waiting
    }

    /// Asks for the bot's move in `run`, which has had `at` actions applied.
    pub fn ask(&mut self, run: Run, at: usize) {
        let worker = self.worker.get_or_insert_with(Worker::start);
        self.waiting = worker.requests.send((at, run)).is_ok();
    }

    /// The answer, once there is one: the action count it was asked at, and the advice.
    pub fn take(&mut self) -> Option<(usize, Option<(RunAction, Why)>)> {
        let worker = self.worker.as_ref()?;
        match worker.answers.try_recv() {
            Ok(answer) => {
                self.waiting = false;
                Some((answer.at, answer.advice))
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.waiting = false;
                self.worker = None;
                None
            }
        }
    }
}

impl Worker {
    fn start() -> Self {
        let (requests, inbox) = channel::<(usize, Run)>();
        let (outbox, answers) = channel();
        thread::spawn(move || {
            let bot = Bot::champion(PHONE);
            while let Ok((at, run)) = inbox.recv() {
                let advice = advise(&bot, &run);
                if outbox.send(Answer { at, advice }).is_err() {
                    break;
                }
            }
        });
        Self { requests, answers }
    }
}

/// The bot's action at `run`'s position and why; `None` when the run is over or nothing is
/// accepted (as `sts2_web_bot::choose`).
fn advise(bot: &Bot, run: &Run) -> Option<(RunAction, Why)> {
    let mut legal = run.legal_actions();
    legal.retain(|a| run.accepts(a));
    if legal.is_empty() {
        return None;
    }
    Some(bot.advise(run.state(), &run.pending_decision(), &legal))
}

/// What to show for the bot's `advice` at the position `app` is in: what to press, then why.
#[must_use]
pub fn words(app: &App, advice: Option<&(RunAction, Why)>) -> String {
    let Some((action, why)) = advice else {
        return "The bot has no advice here.".to_owned();
    };
    format!("Bot: {} ({})", what(app, action), why.line())
}

/// The action as a key and a row of the screen, or in words where no row says it.
fn what(app: &App, action: &RunAction) -> String {
    if let (Some(combat), RunAction::Combat(combat_action)) = (app.combat(), action) {
        let state = combat.state();
        return match combat_action {
            Action::PlayCard { card, target } => {
                let place = state.player.hand.iter().position(|c| c == card);
                let card_name = names::card(state, *card);
                let at = target.map_or_else(String::new, |t| {
                    let slot = state.enemies.iter().position(|e| *e == t);
                    format!(
                        " on {}{}",
                        slot.map_or_else(String::new, |s| format!("{} ", s + 1)),
                        names::creature(state, t)
                    )
                });
                place.map_or_else(
                    || format!("play {card_name}{at}"),
                    |i| format!("play {} {card_name}{at}", i + 1),
                )
            }
            Action::UsePotion { slot, target } => {
                let key = POTION_KEYS
                    .get(*slot)
                    .map_or_else(String::new, |k| format!("{k} "));
                let potion = app
                    .run()
                    .potions()
                    .get(*slot)
                    .copied()
                    .flatten()
                    .map_or_else(|| "a potion".to_owned(), names::potion);
                let at = target.map_or_else(String::new, |t| {
                    format!(" on {}", names::creature(state, t))
                });
                format!("use {key}{potion}{at}")
            }
            Action::EndTurn => "end the turn (e)".to_owned(),
            Action::Resolve(Decision::Pick(card)) => format!("pick {}", names::card(state, *card)),
            Action::Resolve(Decision::Done) => "confirm the choice".to_owned(),
            Action::Resolve(Decision::Unpick(card)) => {
                format!("put back {}", names::card(state, *card))
            }
            Action::Resolve(Decision::Cards(cards)) => {
                format!("pick {}", names::cards(state, cards))
            }
        };
    }
    let items = app.menu();
    let row = items
        .iter()
        .filter(|i| i.enabled)
        .enumerate()
        .find(|(_, i)| i.action == *action);
    if let Some((n, item)) = row {
        return format!("{} {}", app.paging().key(n + 1), item.label);
    }
    // A card of a reward that is not open yet: the reward's row first.
    if let (RunAction::PickCard { reward, card }, RunDecision::Rewards(rewards)) =
        (action, app.run().pending_decision())
        && let Some(Reward::Card(cards)) = rewards.get(*reward)
        && let Some(&picked) = cards.cards.get(*card)
    {
        let open = items
            .iter()
            .filter(|i| i.enabled)
            .position(|i| i.open == Some(*reward));
        let name = names::run_card(picked);
        return open.map_or_else(
            || format!("take {name}"),
            |n| {
                format!(
                    "open the card reward ({}), take {name}",
                    app.paging().key(n + 1)
                )
            },
        );
    }
    match action {
        RunAction::UsePotion(slot) => {
            let key = POTION_KEYS.get(*slot).copied().unwrap_or('z');
            format!("use the potion on {key}")
        }
        RunAction::SkipReward(_) => "skip the card reward (s)".to_owned(),
        other => format!("{other:?}"),
    }
}
