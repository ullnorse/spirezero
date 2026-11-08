//! The rooms' input: what a tap, a digit or Esc means on a screen that is a list of choices,
//! as [`Effect`]s, and the two-step rules (a card reward opened as a screen of its own, a
//! button that asks before it throws something away).
//!
//! The engine decides what is legal ([`menu::items`] marks each choice from `legal_actions`);
//! this only turns the input into the action to apply.

use sts2_core::run::{Run, RunAction, RunDecision};

use super::{Effect, Ui, done, pass};
use crate::describe::Describer;
use crate::menu::{self, MenuItem, ShopSection};
use crate::model::{Input, Key, Screen};
use crate::rooms::{self, Asking, END_BUTTONS, PAGE, Paging};

/// What the client holds about the room on show.
#[derive(Clone, Debug)]
pub struct State {
    /// The card reward (its index among the rewards) open as its own screen.
    open: Option<usize>,
    /// The shop section open as the shop's second level.
    section: Option<ShopSection>,
    /// A button pressed once that cannot be undone, waiting for its second press.
    asking: Option<Asking>,
    /// Whether the buttons that throw something away ask first (the setting).
    confirmations: bool,
    /// The whole shop on one level, no sections (a client that shows all its stock at once).
    flat_shop: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            open: None,
            section: None,
            asking: None,
            confirmations: true,
            flat_shop: false,
        }
    }
}

impl Ui {
    /// Whether buttons that throw something away (Leave with rewards left, Skip) ask first;
    /// buying and removing always do.
    pub const fn set_confirmations(&mut self, on: bool) {
        self.room.confirmations = on;
    }

    #[must_use]
    pub const fn confirmations(&self) -> bool {
        self.room.confirmations
    }

    /// Whether the shop shows its whole stock on one level ([`menu::shop_flat`]) instead of
    /// its sections then their items (off by default).
    pub const fn set_flat_shop(&mut self, on: bool) {
        self.room.flat_shop = on;
        if on {
            self.room.section = None;
        }
    }

    #[must_use]
    pub const fn flat_shop(&self) -> bool {
        self.room.flat_shop
    }

    /// The card reward open as its own screen, while it is still there to answer.
    #[must_use]
    pub fn open_reward(&self, run: &Run) -> Option<usize> {
        let i = self.room.open?;
        match run.pending_decision() {
            RunDecision::Rewards(rewards)
                if matches!(rewards.get(i), Some(sts2_core::run::Reward::Card(_))) =>
            {
                Some(i)
            }
            _ => None,
        }
    }

    /// The shop section open as the shop's second level, while it is still there with items.
    #[must_use]
    pub fn open_section(&self, run: &Run) -> Option<ShopSection> {
        let section = self.room.section?;
        (matches!(run.pending_decision(), RunDecision::Shop(_))
            && menu::section_count(run, section) > 0)
            .then_some(section)
    }

    /// The button pressed once, waiting for its second press.
    #[must_use]
    pub const fn asking(&self) -> Option<&Asking> {
        self.room.asking.as_ref()
    }

    /// The room as a screen (`page` is the page of a long list the focus is on); `None` when
    /// the decision is not one the room builder draws.
    #[must_use]
    pub fn room_screen(&self, run: &Run, describer: &Describer, page: usize) -> Option<Screen> {
        self.room_screen_with(run, describer, page, &[])
    }

    /// [`Self::room_screen`] with the room's receipt, which the map shows.
    #[must_use]
    pub fn room_screen_with(
        &self,
        run: &Run,
        describer: &Describer,
        page: usize,
        receipt: &[String],
    ) -> Option<Screen> {
        let mut screen = rooms::screen(
            run,
            describer,
            &rooms::View {
                open: self.open_reward(run),
                section: self.open_section(run),
                flat_shop: self.room.flat_shop,
                asking: self.room.asking.as_ref(),
                page,
                receipt,
            },
        )?;
        screen.top = Some(self.top_bar(run, describer));
        Some(screen)
    }

    /// Handles `input` on the run's end screen: a tap on a button or its key.
    #[must_use]
    pub fn end_input(&self, input: &Input) -> Vec<Effect> {
        let hit = match *input {
            Input::Tap(id) => (id.0 as usize)
                .checked_sub(1)
                .and_then(|at| END_BUTTONS.get(at)),
            Input::Key(Key::Char(c)) => END_BUTTONS.iter().find(|(key, ..)| *key == c),
            _ => None,
        };
        hit.map_or_else(pass, |&(_, _, action)| vec![Effect::End(action)])
    }

    /// The room's choices, with the reward open as it is.
    #[must_use]
    pub fn room_items(&self, run: &Run, describer: &Describer) -> Vec<MenuItem> {
        menu::shop_items_for(
            run,
            menu::items(run, describer, self.open_reward(run)),
            self.open_section(run),
            self.room.flat_shop,
        )
    }

    /// Handles `input` on a room's screen. `page` is the page of a long list the focus is on,
    /// which a digit picks from.
    pub fn room_input(
        &mut self,
        run: &Run,
        describer: &Describer,
        page: usize,
        input: &Input,
    ) -> Vec<Effect> {
        if let Some(asking) = self.room.asking.take() {
            return self.answer(asking, input);
        }
        match *input {
            Input::Tap(id) if id == rooms::BACK => self.back(run),
            Input::Tap(id) => self.pick(run, describer, id.0 as usize, None),
            Input::Key(Key::Digit(d)) => {
                let digit = usize::from(d);
                let items = self.room_items(run, describer);
                let index = Paging::of(&items)
                    .pick(page, digit)
                    .unwrap_or(page * PAGE + if digit == 0 { PAGE } else { digit });
                self.pick(run, describer, index, Some(Key::Digit(d)))
            }
            Input::Key(Key::Char('s')) if self.open_reward(run).is_some() => {
                let items = self.room_items(run, describer);
                let skip = items
                    .iter()
                    .filter(|i| i.enabled)
                    .position(|i| i.open.is_none() && matches!(i.action, RunAction::SkipReward(_)));
                skip.map_or_else(
                    || vec![Effect::Error("that is not on offer now".to_owned())],
                    |at| self.pick(run, describer, at + 1, Some(Key::Char('s'))),
                )
            }
            Input::Back => self.back(run),
            _ => pass(),
        }
    }

    /// The second press: the same button again, Enter or Space takes it; the arrows leave it
    /// asking; anything else puts it back.
    fn answer(&mut self, asking: Asking, input: &Input) -> Vec<Effect> {
        let yes = match input {
            Input::Primary | Input::Key(Key::Enter | Key::Space) => true,
            Input::Tap(id) => id.0 as usize == asking.entry,
            Input::Key(key) => Some(*key) == asking.key,
            _ => false,
        };
        if matches!(
            input,
            Input::Key(Key::Up | Key::Down | Key::Left | Key::Right)
        ) {
            self.room.asking = Some(asking);
            return done();
        }
        if yes {
            vec![Effect::Apply(asking.action)]
        } else {
            done()
        }
    }

    /// Picks the `index`th (1-based) enabled choice: opens a card reward, asks first, or
    /// applies.
    fn pick(
        &mut self,
        run: &Run,
        describer: &Describer,
        index: usize,
        key: Option<Key>,
    ) -> Vec<Effect> {
        let items = self.room_items(run, describer);
        // The setting switches off the asking for rewards left behind; buying always asks.
        let asks = |action: &RunAction| {
            self.room.confirmations
                || !matches!(action, RunAction::Proceed | RunAction::SkipReward(_))
        };
        let item = index
            .checked_sub(1)
            .and_then(|at| items.iter().filter(|i| i.enabled).nth(at));
        match item {
            Some(MenuItem {
                section: Some((section, _)),
                ..
            }) => {
                self.room.section = Some(*section);
                done()
            }
            Some(MenuItem { open: Some(i), .. }) => {
                self.room.open = Some(*i);
                done()
            }
            Some(MenuItem {
                action,
                warning: Some(what),
                ..
            }) if asks(action) => {
                self.room.asking = Some(Asking {
                    action: action.clone(),
                    what: what.clone(),
                    entry: index,
                    key,
                });
                done()
            }
            Some(item) => vec![Effect::Apply(item.action.clone())],
            None => vec![Effect::Error(format!("no option {index}"))],
        }
    }

    /// Esc: closes the card reward's screen, cancels a card selection the engine lets the
    /// player cancel, or leaves the shop.
    fn back(&mut self, run: &Run) -> Vec<Effect> {
        if self.open_reward(run).is_some() {
            self.room.open = None;
            return done();
        }
        if self.open_section(run).is_some() {
            self.room.section = None;
            return done();
        }
        let leave = match run.pending_decision() {
            RunDecision::CardSelect(_) => RunAction::CancelSelect,
            RunDecision::Shop(_) => RunAction::Proceed,
            _ => return pass(),
        };
        if run.legal_actions().contains(&leave) {
            vec![Effect::Apply(leave)]
        } else {
            done()
        }
    }

    /// The room's memory moves on after an action applied: a card reward taken or skipped
    /// closes its screen.
    pub(super) fn room_applied(&mut self, run: &Run) {
        if self.open_reward(run).is_none() {
            self.room.open = None;
        }
        if self.open_section(run).is_none() {
            self.room.section = None;
        }
    }
}
