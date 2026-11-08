//! The keys: the layers of bindings that apply now, one key press, and the actions the
//! screen under the keys carries out.

use ratatui::crossterm::event::KeyCode;

use super::{Mode, Panel, Shell};
use crate::app::Overlay;
use crate::hit::{Pane, UiAction};
use crate::keys::{self, Command, Layer};
use crate::settings::Row;

impl Shell {
    /// Whether keys type into the seed (so letters are not commands).
    const fn typing(&self) -> bool {
        matches!(&self.mode, Mode::Select(s) if s.is_editing_seed())
    }

    /// The key bindings that apply now, the first match winning: what [`Shell::handle_key`]
    /// reads and the help lists.
    #[must_use]
    pub fn layers(&self) -> Vec<Layer> {
        match self.panel {
            Some(Panel::Help(_)) => return vec![keys::SHELL_TYPING, keys::HELP],
            Some(Panel::Settings(_)) => return vec![keys::SHELL_TYPING, keys::SETTINGS],
            None => {}
        }
        let mut layers = vec![keys::shell_layer(self.typing())];
        match &self.mode {
            Mode::Select(select) => layers.extend(select.layers()),
            Mode::Play(app) => layers.extend(app.layers()),
        }
        layers
    }

    /// The layers of the screen under the help: what the help lists.
    #[must_use]
    pub fn help_layers(&self) -> Vec<Layer> {
        let mut layers = vec![keys::shell_layer(self.typing())];
        match &self.mode {
            Mode::Select(select) => layers.extend(select.layers()),
            Mode::Play(app) => layers.extend(app.layers()),
        }
        layers
    }

    /// Handles one key press.
    pub fn handle_key(&mut self, key: KeyCode) {
        if self.boss {
            self.boss = false;
            return;
        }
        let Some((_, binding)) = keys::lookup(&self.layers(), key, &self.settings.boss_keys) else {
            return;
        };
        match (binding.command, self.panel) {
            (Command::Boss, _) => self.boss = true,
            (Command::Help, _) => self.panel = Some(Panel::Help(0)),
            (Command::Settings, _) => self.panel = Some(Panel::Settings(0)),
            (Command::Close, _) => self.panel = None,
            (Command::Move(by), Some(Panel::Help(scroll))) => self.scroll_help(scroll, by),
            (Command::Move(by), Some(Panel::Settings(row))) => {
                let n = Row::ALL.len().cast_signed();
                let row = (row.cast_signed() + by).rem_euclid(n).cast_unsigned();
                self.panel = Some(Panel::Settings(row));
            }
            (Command::Cycle(forward), Some(Panel::Settings(row))) => {
                self.change_setting(row, forward);
            }
            (_, Some(_)) => {}
            (command, None) => {
                let request = match &mut self.mode {
                    Mode::Select(select) => {
                        select.command(command, key);
                        None
                    }
                    Mode::Play(app) => {
                        app.handle_key(key);
                        app.take_request()
                    }
                };
                // The run's menu entries the shell owns (the help, the settings).
                if let Some(action) = request {
                    self.perform(action);
                }
            }
        }
        self.settle();
    }

    /// Asks the bot what it would do at the run's position; the answer shows up under the
    /// choices when the bot has it (`poll_hint`), and the screen answers keys meanwhile.
    fn ask_bot(&mut self) {
        let Mode::Play(app) = &mut self.mode else {
            return;
        };
        if app.is_over() {
            return;
        }
        self.hinter.ask(app.run().clone(), app.actions().len());
        app.set_hint("Bot: thinking…".to_owned());
    }

    /// Whether the bot is working on an answer: the event loop looks again soon.
    #[must_use]
    pub const fn thinking(&self) -> bool {
        self.hinter.thinking()
    }

    /// Puts the bot's answer on the screen once it is there, if the run is still where it was
    /// asked.
    pub fn poll_hint(&mut self) {
        let Some((at, advice)) = self.hinter.take() else {
            return;
        };
        if let Mode::Play(app) = &mut self.mode
            && app.actions().len() == at
        {
            let text = crate::hint::words(app, advice.as_ref());
            app.set_hint(text);
        }
    }

    /// Carries out `action` on the screen shown.
    pub fn perform(&mut self, action: UiAction) {
        match (action, self.panel) {
            (UiAction::Help | UiAction::Settings, _) => {
                // Picked from the run's menu: the menu makes way.
                if let Mode::Play(app) = &mut self.mode
                    && app.overlay() == Some(Overlay::Menu)
                {
                    app.perform(UiAction::Back);
                }
                self.panel = Some(if action == UiAction::Help {
                    Panel::Help(0)
                } else {
                    Panel::Settings(0)
                });
            }
            (UiAction::Hint, None) => self.ask_bot(),
            (UiAction::Close | UiAction::Back, Some(_)) => self.panel = None,
            (UiAction::Setting(row), Some(Panel::Settings(_))) => {
                self.panel = Some(Panel::Settings(row));
                self.change_setting(row, true);
            }
            (_, Some(_)) => {}
            (action, None) => match &mut self.mode {
                Mode::Select(select) => select.perform(action),
                Mode::Play(app) => app.perform(action),
            },
        }
        self.settle();
    }

    /// Scrolls the help from `scroll` by `by` rows, no further than the last frame could show.
    pub(super) fn scroll_help(&mut self, scroll: usize, by: isize) {
        let max = self
            .hits
            .scroll_limit(Pane::Help)
            .map_or(usize::MAX, |(_, max)| usize::try_from(max).unwrap_or(0));
        let scroll = usize::try_from(scroll.cast_signed() + by).unwrap_or(0);
        self.panel = Some(Panel::Help(scroll.min(max)));
    }
}
