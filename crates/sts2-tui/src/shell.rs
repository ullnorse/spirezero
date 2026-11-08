//! The top-level state and its event loop: the character select that opens the game, then a
//! run, the settings and the autosave. No drawing and no game rules here either.
//!
//! The pieces live beside it: [`select`] (the character select wrapper), [`input`] (key
//! layers and presses) and [`mouse`] (clicks, drags and the wheel).

use std::rc::Rc;

use ratatui::layout::Position;

use sts2_view::launch::Request;
pub use sts2_view::launch::SELECT_ORDER;

use crate::app::{App, random_seed};
use crate::hint::Hinter;
use crate::hit::Hits;
use crate::names;
use crate::session::{Save, Session};
use crate::settings::Row;
use crate::settings::{Settings, Theme};
use crate::text::Localization;

/// What the TUI is showing.
#[derive(Clone, Debug)]
pub enum Mode {
    Select(Box<CharacterSelect>),
    Play(Box<App>),
}

/// A window over the screen that takes the keys while it is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    /// The keys and mouse actions of the screen under it (`?`), scrolled this many rows.
    Help(usize),
    /// The settings (`o`), with the selected row.
    Settings(usize),
}

/// The whole TUI: the select screen or a run, the text both show, the settings, the
/// autosave, and the mouse: where it is and the regions of the last frame drawn.
#[derive(Clone, Debug)]
pub struct Shell {
    mode: Mode,
    text: Rc<Localization>,
    quit: bool,
    hits: Hits,
    pointer: Option<Position>,
    press: Option<mouse::Press>,
    settings: Settings,
    /// The autosave; `None` keeps everything in memory (tests, no home directory).
    session: Option<Session>,
    panel: Option<Panel>,
    /// The boss screen is up: any key or click takes it down.
    boss: bool,
    settings_changed: bool,
    /// The bot behind the hint key, on a thread of its own.
    hinter: Hinter,
    /// The palette the terminal (`COLORFGBG`) or the command line (`--theme`) asks for, with
    /// the theme setting as it was then: it holds until that setting is changed.
    palette: Option<(Theme, Theme)>,
}

impl Shell {
    fn new(
        mode: Mode,
        text: Rc<Localization>,
        settings: Settings,
        session: Option<Session>,
    ) -> Self {
        let mut shell = Self {
            mode,
            text,
            quit: false,
            hits: Hits::default(),
            pointer: None,
            press: None,
            settings,
            session,
            panel: None,
            boss: false,
            settings_changed: false,
            hinter: Hinter::default(),
            palette: None,
        };
        if let Mode::Play(app) = &mut shell.mode {
            app.set_confirmations(shell.settings.confirmations);
        }
        shell.settle();
        shell
    }

    /// Opens on the character select, with the default settings and no autosave.
    #[must_use]
    pub fn select(text: Rc<Localization>) -> Self {
        let select = CharacterSelect::new(SELECT_ORDER[0], 0, Rc::clone(&text));
        Self::new(
            Mode::Select(Box::new(select)),
            text,
            Settings::default(),
            None,
        )
    }

    /// Opens straight into `app` (the command-line flags named the run), with the default
    /// settings and no autosave.
    #[must_use]
    pub fn play(app: App) -> Self {
        let text = app.describer().localization();
        Self::new(Mode::Play(Box::new(app)), text, Settings::default(), None)
    }

    /// The game as launched: the character select (with the last choices, and the saved run as
    /// Continue), or `app` when the command line named a run; `session` keeps the save.
    #[must_use]
    pub fn launch(
        text: Rc<Localization>,
        settings: Settings,
        session: Option<Session>,
        app: Option<App>,
    ) -> Self {
        let mode = if let Some(app) = app {
            Mode::Play(Box::new(app))
        } else {
            let (save, crash) = session
                .as_ref()
                .map_or((None, None), |s| (s.load(), s.take_crash_note()));
            Mode::Select(Box::new(
                CharacterSelect::new(
                    settings.last_character,
                    settings.last_ascension,
                    Rc::clone(&text),
                )
                .with_save(save, crash),
            ))
        };
        Self::new(mode, text, settings, session)
    }

    #[must_use]
    pub const fn mode(&self) -> &Mode {
        &self.mode
    }

    #[must_use]
    pub const fn should_quit(&self) -> bool {
        self.quit
    }

    #[must_use]
    pub const fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Draws in `theme` until the theme setting is changed (the terminal's `COLORFGBG` or
    /// `--theme` speak for the setting's default).
    pub const fn set_default_theme(&mut self, theme: Theme) {
        self.palette = Some((theme, self.settings.theme));
    }

    /// The colours to draw in: the settings', or the default the terminal or the command line
    /// asked for while the setting is as it was loaded.
    #[must_use]
    pub fn palette(&self) -> Theme {
        match self.palette {
            Some((theme, loaded)) if !self.settings.discreet && self.settings.theme == loaded => {
                theme
            }
            _ => self.settings.palette(),
        }
    }

    /// Where the settings and the save are kept, if anywhere.
    #[must_use]
    pub const fn session(&self) -> Option<&Session> {
        self.session.as_ref()
    }

    /// The window open over the screen.
    #[must_use]
    pub const fn panel(&self) -> Option<Panel> {
        self.panel
    }

    /// Whether the boss screen is up.
    #[must_use]
    pub const fn boss(&self) -> bool {
        self.boss
    }

    /// Whether the settings changed since the last call (the main loop sets the mouse).
    pub const fn take_settings_changed(&mut self) -> bool {
        std::mem::replace(&mut self.settings_changed, false)
    }

    /// Applies the run action `action` to the run shown, as the key that makes it would (for
    /// a policy or a tool driving the game), with the autosave.
    pub fn apply(&mut self, action: sts2_core::run::RunAction) {
        if let Mode::Play(app) = &mut self.mode {
            app.apply(action);
        }
        self.settle();
    }

    /// Moves settings row `row` to its next (or previous) value and saves the settings.
    fn change_setting(&mut self, row: usize, forward: bool) {
        if let Some(&row) = Row::ALL.get(row) {
            self.settings.cycle(row, forward);
            self.settings_changed();
        }
    }

    /// Saves the settings and passes them on to what uses them.
    fn settings_changed(&mut self) {
        self.settings_changed = true;
        if let Mode::Play(app) = &mut self.mode {
            app.set_confirmations(self.settings.confirmations);
        }
        if let Some(session) = &self.session
            && let Err(e) = self.settings.save(&session.dirs().settings())
        {
            self.notify(format!("cannot save the settings: {e}"));
        }
    }

    /// Shows `message` on the screen's error line.
    fn notify(&mut self, message: String) {
        match &mut self.mode {
            Mode::Select(select) => select.launch.notify(message),
            Mode::Play(app) => app.notify(message),
        }
    }

    /// Follows up on what the last input asked for: quitting, starting or continuing a run,
    /// going back to the select screen, and the autosave.
    fn settle(&mut self) {
        match &mut self.mode {
            Mode::Select(select) => match select.launch.take_request() {
                Some(Request::Quit) => self.quit = true,
                Some(Request::Continue { earlier }) => self.resume(earlier),
                Some(Request::Start) => self.begin(),
                Some(Request::Settings) => self.panel = Some(Panel::Settings(0)),
                None => {}
            },
            Mode::Play(app) => {
                self.quit = app.should_quit();
                if app.wants_select() {
                    let select = CharacterSelect::new(
                        app.run().state().character,
                        app.run().state().ascension,
                        Rc::clone(&self.text),
                    );
                    self.mode = Mode::Select(Box::new(select));
                }
            }
        }
        if let (Mode::Play(app), Some(session)) = (&mut self.mode, &mut self.session)
            && app.take_changed()
            && let Err(e) = session.autosave(app)
        {
            app.notify(format!("cannot save the run: {e}"));
        }
    }

    /// Starts the run the select screen describes.
    fn begin(&mut self) {
        let Mode::Select(select) = &mut self.mode else {
            return;
        };
        let seed = if select.seed().is_empty() {
            random_seed()
        } else {
            select.seed().to_owned()
        };
        match App::with_text(
            select.character(),
            &seed,
            select.ascension(),
            Rc::clone(&self.text),
        ) {
            Ok(mut app) => {
                app.set_confirmations(self.settings.confirmations);
                let remember = (select.character(), select.ascension());
                self.mode = Mode::Play(Box::new(app));
                if (self.settings.last_character, self.settings.last_ascension) != remember {
                    (self.settings.last_character, self.settings.last_ascension) = remember;
                    self.settings_changed();
                }
                if let Some(session) = &mut self.session {
                    session.forget();
                }
            }
            Err(e) => select.launch.notify(names::run_error(&e)),
        }
    }

    /// Continues the saved run, from its last action or (`earlier`) the one before.
    fn resume(&mut self, earlier: bool) {
        let Mode::Select(select) = &mut self.mode else {
            return;
        };
        let save = select.saved.as_ref().and_then(|s| {
            if earlier {
                s.earlier()
            } else {
                Some(s.clone())
            }
        });
        let Some(save) = save else {
            return;
        };
        let text = Rc::clone(&self.text);
        // A save that crashes the engine on the way must not take the game down with it.
        let restored = crate::session::catch_panic(|| save.restore(text));
        match restored {
            Ok(Ok(restored)) => {
                let mut app = restored.app;
                app.set_confirmations(self.settings.confirmations);
                if !restored.exact {
                    app.notify(
                        "this save was made by another build: the run goes on, but it may differ"
                            .to_owned(),
                    );
                }
                if let Some(session) = &mut self.session {
                    session.resume(&app);
                }
                // The app is new, but the save on disk already holds it.
                let _ = app.take_changed();
                self.mode = Mode::Play(Box::new(app));
            }
            Ok(Err(e)) => {
                select.launch.drop_menu();
                select
                    .launch
                    .notify(format!("the saved run cannot be continued: {e}"));
            }
            Err(_) => {
                select.launch.drop_menu();
                if select.saved.as_ref().and_then(Save::earlier).is_some() {
                    select.launch.offer_earlier();
                }
                select.launch.notify(if earlier {
                    "continuing the saved run crashed".to_owned()
                } else {
                    "continuing the saved run crashed: try b (before the last action)".to_owned()
                });
            }
        }
    }
}

mod input;
mod mouse;
mod select;

pub use select::CharacterSelect;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hit::{Target, UiAction};
    use ratatui::crossterm::event::KeyCode;
    use sts2_core::Character;
    use sts2_core::run::ascension::MAX_ASCENSION;

    fn press(shell: &mut Shell, keys: &str) {
        for c in keys.chars() {
            shell.handle_key(KeyCode::Char(c));
        }
    }

    fn select(shell: &Shell) -> &CharacterSelect {
        match shell.mode() {
            Mode::Select(s) => s,
            Mode::Play(_) => panic!("not on the select screen"),
        }
    }

    #[test]
    fn character_ascension_and_seed_are_chosen_with_keys() {
        let mut shell = Shell::select(Rc::new(Localization::empty()));
        assert_eq!(select(&shell).character(), Character::Ironclad);
        press(&mut shell, "3");
        assert_eq!(select(&shell).character(), Character::Regent);
        for _ in 0..3 {
            shell.handle_key(KeyCode::Right);
        }
        assert_eq!(select(&shell).character(), Character::Ironclad, "wraps");
        shell.handle_key(KeyCode::Left);
        assert_eq!(select(&shell).character(), Character::Defect);
        for _ in 0..20 {
            shell.handle_key(KeyCode::Up);
        }
        assert_eq!(select(&shell).ascension(), MAX_ASCENSION);
        press(&mut shell, "-");
        assert_eq!(select(&shell).ascension(), MAX_ASCENSION - 1);
        // `s` types into the seed, so `q` and digits are letters there.
        press(&mut shell, "sab1q");
        assert!(select(&shell).is_editing_seed());
        assert_eq!(select(&shell).seed(), "AB1Q");
        shell.handle_key(KeyCode::Backspace);
        shell.handle_key(KeyCode::Enter);
        assert!(!select(&shell).is_editing_seed());
        assert_eq!(select(&shell).seed(), "AB1");
        assert!(!shell.should_quit());
    }

    #[test]
    fn each_key_picks_the_character_the_screen_numbers_with_it() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        use ratatui::buffer::Cell;

        for (i, character) in SELECT_ORDER.iter().enumerate() {
            let mut shell = Shell::select(Rc::new(Localization::empty()));
            press(&mut shell, &(i + 1).to_string());
            assert_eq!(select(&shell).character(), *character);
            let mut terminal = Terminal::new(TestBackend::new(160, 40)).expect("backend");
            terminal
                .draw(|f| crate::ui::draw_shell(f, &shell))
                .expect("draw");
            let screen: String = terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .map(Cell::symbol)
                .collect();
            assert!(
                screen.contains(&character.name().to_uppercase()),
                "{} should show {}",
                i + 1,
                character.name()
            );
        }
    }

    #[test]
    fn enter_starts_a_run_with_the_choices() {
        let mut shell = Shell::select(Rc::new(Localization::empty()));
        press(&mut shell, "3");
        press(&mut shell, "+++");
        press(&mut shell, "sregent");
        shell.handle_key(KeyCode::Enter);
        shell.handle_key(KeyCode::Enter);
        let Mode::Play(app) = shell.mode() else {
            panic!("a run should have started");
        };
        assert_eq!(app.run().state().character, Character::Regent);
        assert_eq!(app.run().state().ascension, 3);
        assert_eq!(app.seed(), "REGENT");
    }

    #[test]
    fn no_seed_means_a_random_one() {
        let mut shell = Shell::select(Rc::new(Localization::empty()));
        shell.handle_key(KeyCode::Enter);
        let Mode::Play(app) = shell.mode() else {
            panic!("a run should have started");
        };
        assert_eq!(app.seed().len(), 10);
    }

    #[test]
    fn the_main_menu_is_walked_with_the_arrows() {
        let save = Save {
            character: Character::Silent,
            ascension: 2,
            seed: "RL2S2".to_owned(),
            act: 1,
            floor: 3,
            hp: 50,
            max_hp: 70,
            gold: 120,
            digest: 0,
            previous: None,
            actions: Vec::new(),
        };
        let menu = CharacterSelect::new(Character::Ironclad, 0, Rc::new(Localization::empty()))
            .with_save(Some(save), None);
        let mut shell = Shell::new(
            Mode::Select(Box::new(menu)),
            Rc::new(Localization::empty()),
            Settings::default(),
            None,
        );
        assert_eq!(
            select(&shell).focused(),
            Some(Target::Button(UiAction::Continue)),
            "Continue first"
        );
        shell.handle_key(KeyCode::Down);
        shell.handle_key(KeyCode::Down);
        assert_eq!(
            select(&shell).focused(),
            Some(Target::Button(UiAction::Settings))
        );
        shell.handle_key(KeyCode::Enter);
        assert_eq!(
            shell.panel(),
            Some(Panel::Settings(0)),
            "Enter opens the entry"
        );
        shell.handle_key(KeyCode::Esc);
        shell.handle_key(KeyCode::Up);
        assert_eq!(
            select(&shell).focused(),
            Some(Target::Button(UiAction::NewRun))
        );
        shell.handle_key(KeyCode::Enter);
        assert!(
            !select(&shell).continue_focused(),
            "New run: the character select"
        );
        assert_eq!(select(&shell).focused(), None);
    }

    #[test]
    fn the_end_screen_goes_back_to_the_select() {
        let app = App::with_text(
            Character::Silent,
            "RL2S2",
            2,
            Rc::new(Localization::empty()),
        )
        .expect("run");
        let mut shell = Shell::play(app);
        // Not over yet: `c` is just an unknown key.
        press(&mut shell, "c");
        assert!(matches!(shell.mode(), Mode::Play(_)));
    }
}
