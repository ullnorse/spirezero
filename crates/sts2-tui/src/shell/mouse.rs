//! The mouse: what is under the pointer, clicks, drags, the right button and the wheel, read
//! from the regions of the last frame drawn.

use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Position;
use sts2_core::combat::CardId;

use super::{Mode, Panel, Shell};
use crate::app::{App, Input, Inspect};
use crate::hit::{Hits, Pane, Target, UiAction};

/// A held left button: where it went down, what was there (topmost first), and whether the
/// pointer has left that since (a drag).
#[derive(Clone, Debug)]
pub(super) struct Press {
    pub(super) targets: Vec<Target>,
    pub(super) dragged: bool,
}

impl Shell {
    /// What the pointer is over, in the last frame drawn (a pane or the battlefield is only
    /// for the wheel and drops, so it does not count).
    #[must_use]
    pub fn hover(&self) -> Option<Target> {
        self.pointer.and_then(|p| {
            self.hits
                .at(p)
                .find(|t| !matches!(t, Target::Pane(_) | Target::Battlefield))
        })
    }

    /// Where the mouse was last seen.
    #[must_use]
    pub const fn pointer(&self) -> Option<Position> {
        self.pointer
    }

    /// The regions of the last frame drawn.
    #[must_use]
    pub const fn hits(&self) -> &Hits {
        &self.hits
    }

    /// Keeps the regions of the frame just drawn, for the next mouse event.
    pub fn set_hits(&mut self, hits: Hits) {
        if let Mode::Play(app) = &mut self.mode {
            app.clamp_scrolls(&hits);
        }
        if let (Some(Panel::Help(scroll)), Some((_, max))) =
            (self.panel, hits.scroll_limit(Pane::Help))
        {
            self.panel = Some(Panel::Help(scroll.min(usize::try_from(max).unwrap_or(0))));
        }
        self.hits = hits;
    }

    /// Handles one mouse event, by what the last frame drew under the pointer. Moving over a
    /// choice does nothing (no popup, and the focus stays where the arrows put it); a left
    /// click presses what is there (a card is played, or waits for a click on its target); a press on a card released
    /// elsewhere drops the card there; the right button goes back; the wheel scrolls what is
    /// under the pointer.
    pub fn handle_mouse(&mut self, event: MouseEvent) {
        if self.boss {
            if matches!(event.kind, MouseEventKind::Down(_)) {
                self.boss = false;
            }
            return;
        }
        let pos = Position::new(event.column, event.row);
        self.pointer = Some(pos);
        let here: Vec<Target> = self.hits.at(pos).collect();
        // The pointer moving never moves the focus: a pointer resting on a row (a hand on a
        // trackpad, a stray motion event) would otherwise retarget Enter to the row under it
        // while the player walks the rows with the arrows. The arrows move the focus; a click
        // presses what is under the pointer, and puts the focus there.
        if let (MouseEventKind::Down(MouseButton::Left), Some(&over), Mode::Play(app)) =
            (event.kind, here.first(), &mut self.mode)
        {
            app.focus_on(over);
        }
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.press = Some(Press {
                    targets: here,
                    dragged: false,
                });
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                let Some(press) = &mut self.press else {
                    return;
                };
                if !press.dragged && here.first() != press.targets.first() {
                    press.dragged = true;
                    if let Some(&Target::Card(card)) = press.targets.first() {
                        self.perform(UiAction::Arm(card));
                    }
                }
            }
            // Windows consoles report every release as the left button's.
            MouseEventKind::Up(_) => {
                let Some(press) = self.press.take() else {
                    return;
                };
                match press.targets.first() {
                    Some(&Target::Card(card)) if press.dragged || here != press.targets => {
                        self.drop_card(card, &here);
                    }
                    _ if here == press.targets => self.click(&here),
                    // Pressed on one thing and released on another: nothing, like a button.
                    _ => {}
                }
            }
            MouseEventKind::Down(MouseButton::Right) => self.right_click(&here),
            MouseEventKind::ScrollUp => self.wheel(&here, false),
            MouseEventKind::ScrollDown => self.wheel(&here, true),
            _ => {}
        }
    }

    /// A right click on `targets`: the detail sheet of the card, enemy or choice under it, else
    /// back.
    fn right_click(&mut self, targets: &[Target]) {
        let what = match &self.mode {
            Mode::Play(app) if app.overlay().is_none() && self.panel.is_none() => {
                let fight = app.combat();
                targets.iter().find_map(|&t| match (t, fight) {
                    (Target::Card(id) | Target::Choice(_, id), Some(_)) => Some(Inspect::Card(id)),
                    (Target::Creature(id), Some(c)) if c.state().enemies.contains(&id) => {
                        Some(Inspect::Enemy(id))
                    }
                    (Target::Button(UiAction::Pick(n)), None) => Some(Inspect::Item(n)),
                    (Target::Potion(slot), _) => Some(Inspect::Potion(slot)),
                    (Target::Relic(i), _) => Some(Inspect::Relic(i)),
                    _ => None,
                })
            }
            _ => None,
        };
        match (what, &mut self.mode) {
            (Some(what), Mode::Play(app)) => app.open_inspect(what),
            _ => self.perform(UiAction::Back),
        }
    }

    /// The run's input, if a run is shown.
    fn app(&self) -> Option<&App> {
        match &self.mode {
            Mode::Play(app) => Some(app),
            Mode::Select(_) => None,
        }
    }

    /// Whether a card or potion is waiting for its target.
    fn aiming(&self) -> bool {
        self.app()
            .is_some_and(|a| matches!(a.input(), Input::Target(_) | Input::Armed(_)))
    }

    /// A click on `targets` (topmost first): the first that does something when clicked.
    fn click(&mut self, targets: &[Target]) {
        let aiming = self.aiming();
        let armed = self
            .app()
            .is_some_and(|a| matches!(a.input(), Input::Armed(_)));
        let action = targets.iter().find_map(|&t| match t {
            Target::Creature(id) if aiming => Some(UiAction::Target(id)),
            Target::Battlefield if armed => Some(UiAction::PlayArmed),
            _ => t.click(),
        });
        match (action, self.app()) {
            (Some(action), _) => self.perform(action),
            (None, _) if self.panel.is_some() => {}
            // A click on nothing answers no to a question and drops a selection.
            (None, Some(app)) if app.confirm_prompt().is_some() => self.perform(UiAction::No),
            (None, Some(app)) if app.overlay().is_none() && *app.input() != Input::Idle => {
                self.perform(UiAction::Back);
            }
            (None, _) => {}
        }
    }

    /// `card` was dragged from the hand and released over `targets`: played on the creature
    /// there, or (if it needs no target) anywhere on the battlefield; elsewhere it goes back.
    fn drop_card(&mut self, card: CardId, targets: &[Target]) {
        let armed = self.app().is_some_and(|a| {
            matches!(a.input(), Input::Armed(c) | Input::Target(crate::app::Pending::Card(c)) if *c == card)
        });
        if !armed {
            self.perform(UiAction::Arm(card));
        }
        let creature = targets.iter().find_map(|t| match t {
            Target::Creature(id) => Some(*id),
            _ => None,
        });
        if let Some(id) = creature {
            self.perform(UiAction::Target(id));
        } else if targets.contains(&Target::Battlefield) {
            self.perform(UiAction::PlayArmed);
        } else {
            self.perform(UiAction::Back);
        }
    }

    /// The wheel over `targets`: the map or the ascension under the pointer, else the open list.
    fn wheel(&mut self, targets: &[Target], down: bool) {
        let pane = targets.iter().find_map(|t| match t {
            Target::Pane(p) => Some(*p),
            _ => None,
        });
        let by: isize = if down { 1 } else { -1 };
        match (&self.mode, pane) {
            _ if matches!(self.panel, Some(Panel::Help(_))) => {
                if let Some(Panel::Help(scroll)) = self.panel {
                    self.scroll_help(scroll, 3 * by);
                }
            }
            _ if self.panel.is_some() => {}
            (Mode::Select(_), Some(Pane::Ascension)) => self.perform(UiAction::Ascension(!down)),
            (Mode::Select(_), _) => {}
            // The wheel never answers a question.
            (Mode::Play(app), _) if app.confirm_prompt().is_some() => {}
            (Mode::Play(_), Some(Pane::Map)) => self.perform(UiAction::ScrollMap(2 * by)),
            (Mode::Play(_), _) => self.perform(UiAction::Scroll(3 * by)),
        }
    }
}
