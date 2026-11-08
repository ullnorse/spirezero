//! The screens before a run, as [`Screen`]s: the main menu while a saved run waits, the character
//! and ascension select, and the settings (spec 2.1, 2.2, 2.16).
//!
//! [`Launch`] is their state machine: it takes [`Input`]s (and [`Key`]s) and asks the client for
//! what it cannot do itself with a [`Request`] (start, continue, quit, open the settings). The
//! saved run, the crash note and the settings file are the client's; it hands `Launch` only the
//! words to show.

use sts2_core::Character;
use sts2_core::run::ascension::MAX_ASCENSION;
use sts2_core::run::neow;

use crate::describe::Describer;
use crate::model::{Block, Button, ButtonId, ButtonState, InfoRef, Input, Key, Screen};
use crate::text::{self, Rich, RichSpan, Role};

/// The character-select screen's order, as the game shows it. [`Character::ALL`] keeps
/// `ModelDb.AllCharacters`' order instead, which rolls (Orobas) depend on.
pub const SELECT_ORDER: [Character; 5] = [
    Character::Ironclad,
    Character::Silent,
    Character::Regent,
    Character::Necrobinder,
    Character::Defect,
];

/// The longest custom seed that can be typed.
pub const MAX_SEED_LEN: usize = 24;

/// The main menu's rows (`Continue` is the first row of the menu, the others follow).
pub const CONTINUE: ButtonId = ButtonId(1);
pub const EARLIER: ButtonId = ButtonId(2);
pub const NEW_RUN: ButtonId = ButtonId(3);
pub const SETTINGS: ButtonId = ButtonId(4);
pub const QUIT: ButtonId = ButtonId(5);
/// The select's ascension row and its `-` and `+`, and the seed row.
pub const ASCENSION: ButtonId = ButtonId(22);
pub const ASCENSION_DOWN: ButtonId = ButtonId(20);
pub const ASCENSION_UP: ButtonId = ButtonId(21);
pub const SEED: ButtonId = ButtonId(23);

/// What the saved run's line says, worded by the client.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SavedRun {
    /// `Silent · Ascension 2`.
    pub who: String,
    /// `Act 1 · Floor 3 · ♥ 50/70 · ◉ 120`.
    pub place: String,
}

/// What the launch screens ask the client for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    Start,
    /// Continue the saved run, from its last action or the one before.
    Continue {
        earlier: bool,
    },
    Quit,
    Settings,
}

/// The main menu and the character select.
#[derive(Clone, Debug, Default)]
pub struct Launch {
    /// Index into [`SELECT_ORDER`].
    character: usize,
    ascension: u8,
    /// The custom seed typed so far; empty means a random one.
    seed: String,
    editing: bool,
    error: Option<String>,
    request: Option<Request>,
    saved: Option<SavedRun>,
    /// Whether the main menu shows (until the player starts choosing a new run).
    menu: bool,
    /// The main menu's highlighted row, as [`Launch::menu_rows`] lists them.
    focus: usize,
    crash: Option<String>,
    /// The saved run can also be continued from before its last action.
    earlier: bool,
}

impl Launch {
    /// The select, with `character` highlighted.
    #[must_use]
    pub fn new(character: Character, ascension: u8) -> Self {
        Self {
            character: SELECT_ORDER
                .iter()
                .position(|&c| c == character)
                .unwrap_or(0),
            ascension: ascension.min(MAX_ASCENSION),
            ..Self::default()
        }
    }

    /// Offers `saved` as Continue (highlighted, so Enter continues), with the crash note's text
    /// when the last session crashed, and whether the run can go on from before its last action.
    #[must_use]
    pub fn with_save(
        mut self,
        saved: Option<SavedRun>,
        crash: Option<String>,
        earlier: bool,
    ) -> Self {
        self.menu = saved.is_some();
        self.saved = saved;
        self.crash = crash;
        self.earlier = earlier;
        self
    }

    #[must_use]
    pub const fn character(&self) -> Character {
        SELECT_ORDER[self.character]
    }

    #[must_use]
    pub const fn ascension(&self) -> u8 {
        self.ascension
    }

    /// The custom seed typed so far (empty: a random seed is rolled at the start).
    #[must_use]
    pub fn seed(&self) -> &str {
        &self.seed
    }

    #[must_use]
    pub const fn is_editing_seed(&self) -> bool {
        self.editing
    }

    /// Whether the main menu shows.
    #[must_use]
    pub const fn menu_shown(&self) -> bool {
        self.menu && self.saved.is_some()
    }

    /// The main menu's highlighted row.
    #[must_use]
    pub fn focused(&self) -> Option<ButtonId> {
        let rows = self.menu_rows();
        self.menu_shown()
            .then(|| rows[self.focus.min(rows.len() - 1)])
    }

    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Shows `message` on the screen's status line.
    pub fn notify(&mut self, message: String) {
        self.error = Some(message);
    }

    /// Asks the client to open the settings.
    pub const fn request_settings(&mut self) {
        self.request = Some(Request::Settings);
    }

    /// The saved run no longer offers Continue (it could not be restored).
    pub const fn drop_menu(&mut self) {
        self.menu = false;
    }

    /// Offers the run from before its last action (continuing crashed).
    pub const fn offer_earlier(&mut self) {
        self.earlier = true;
    }

    #[must_use]
    pub const fn can_continue_earlier(&self) -> bool {
        self.earlier
    }

    /// What the last input asked the client for.
    pub const fn take_request(&mut self) -> Option<Request> {
        self.request.take()
    }

    /// The main menu's rows in order.
    fn menu_rows(&self) -> Vec<ButtonId> {
        let mut rows = vec![CONTINUE];
        if self.earlier {
            rows.push(EARLIER);
        }
        rows.extend([NEW_RUN, SETTINGS, QUIT]);
        rows
    }

    const fn move_character(&mut self, by: isize) {
        let n = SELECT_ORDER.len().cast_signed();
        self.character = (self.character.cast_signed() + by)
            .rem_euclid(n)
            .cast_unsigned();
    }

    const fn move_ascension(&mut self, up: bool) {
        self.ascension = if up {
            if self.ascension < MAX_ASCENSION {
                self.ascension + 1
            } else {
                MAX_ASCENSION
            }
        } else {
            self.ascension.saturating_sub(1)
        };
    }

    /// Handles `input`.
    pub fn input(&mut self, input: &Input) {
        self.error = None;
        match *input {
            Input::Tap(id) => self.tap(id),
            Input::Key(key) => self.key(key),
            Input::Primary => self.tap_primary(),
            Input::Back => self.back(),
            Input::Page(_) | Input::Info(_) | Input::Arm(_) | Input::Drop => {}
        }
    }

    const fn back(&mut self) {
        if self.editing {
            self.editing = false;
        }
    }

    const fn tap_primary(&mut self) {
        if self.menu_shown() {
            return;
        }
        self.editing = false;
        self.request = Some(Request::Start);
    }

    const fn tap(&mut self, id: ButtonId) {
        if self.menu_shown() {
            match id {
                CONTINUE => self.request = Some(Request::Continue { earlier: false }),
                EARLIER if self.earlier => self.request = Some(Request::Continue { earlier: true }),
                NEW_RUN => self.menu = false,
                SETTINGS => self.request = Some(Request::Settings),
                QUIT => self.request = Some(Request::Quit),
                _ => {}
            }
            return;
        }
        match id {
            ButtonId(n @ 1..=5) => {
                self.editing = false;
                self.character = n as usize - 1;
            }
            ASCENSION_DOWN => self.move_ascension(false),
            ASCENSION_UP | ASCENSION => self.move_ascension(true),
            SEED => self.editing = !self.editing,
            _ => {}
        }
    }

    /// Handles a key the screen did not print on a row's button as a global (the arrows, the
    /// seed's typing, the letters the screen prints).
    pub fn key(&mut self, key: Key) {
        self.error = None;
        if self.editing {
            match key {
                Key::Char(c) if self.seed.len() < MAX_SEED_LEN && c.is_ascii_graphic() => {
                    self.seed.push(c.to_ascii_uppercase());
                }
                Key::Digit(d) if self.seed.len() < MAX_SEED_LEN => {
                    self.seed.push(char::from(b'0' + d));
                }
                Key::Backspace => {
                    self.seed.pop();
                }
                Key::Enter | Key::Esc => self.editing = false,
                _ => {}
            }
            return;
        }
        if self.menu_shown() {
            self.menu_key(key);
        } else {
            self.select_key(key);
        }
    }

    fn menu_key(&mut self, key: Key) {
        let rows = self.menu_rows();
        let n = rows.len().cast_signed();
        let step = |at: usize, by: isize| (at.cast_signed() + by).rem_euclid(n).cast_unsigned();
        match key {
            Key::Up | Key::Left => self.focus = step(self.focus, -1),
            Key::Down | Key::Right => self.focus = step(self.focus, 1),
            Key::Enter | Key::Space => {
                let id = rows[self.focus.min(rows.len() - 1)];
                self.tap(id);
            }
            Key::Digit(d @ 1..=9) => {
                if let Some(&id) = rows.get(usize::from(d) - 1) {
                    self.tap(id);
                }
            }
            Key::Char('c') => self.tap(CONTINUE),
            Key::Char('b') => self.tap(EARLIER),
            Key::Char('n') => self.tap(NEW_RUN),
            Key::Char('o') => self.tap(SETTINGS),
            Key::Char('q') | Key::Esc => self.tap(QUIT),
            _ => {}
        }
    }

    fn select_key(&mut self, key: Key) {
        match key {
            Key::Left | Key::Char('h') => self.move_character(-1),
            Key::Right | Key::Char('l') => self.move_character(1),
            Key::Up | Key::Char('+' | 'k') => self.move_ascension(true),
            Key::Down | Key::Char('-' | 'j') => self.move_ascension(false),
            Key::Digit(d @ 1..=5) => self.tap(ButtonId(u32::from(d))),
            Key::Char('s') => self.editing = true,
            Key::Char('r') => self.seed.clear(),
            Key::Enter | Key::Space => self.tap_primary(),
            Key::Char('q') | Key::Esc => self.request = Some(Request::Quit),
            _ => {}
        }
    }

    /// The screen shown now.
    #[must_use]
    pub fn screen(&self, describer: &Describer) -> Screen {
        let mut screen = if self.menu_shown() {
            self.menu_screen()
        } else {
            self.select_screen(describer)
        };
        screen.status = self.error.as_deref().map(|e| role(Role::Lethal, e));
        screen
    }

    fn menu_screen(&self) -> Screen {
        let mut text = Vec::new();
        if let Some(saved) = &self.saved {
            text.push(Rich(vec![RichSpan {
                fx: None,
                role: Role::Plain,
                bold: true,
                text: saved.who.clone(),
            }]));
            text.push(role(Role::Muted, &saved.place));
        }
        if let Some(note) = &self.crash {
            text.push(role(
                Role::Attack,
                &format!("The game crashed last time ({note}). The run is saved up to the last good action."),
            ));
        }
        let focus = self.focused();
        let rows = self
            .menu_rows()
            .into_iter()
            .enumerate()
            .map(|(at, id)| {
                let label = match id {
                    CONTINUE => "Continue",
                    EARLIER => "Before the last action",
                    NEW_RUN => "New run",
                    SETTINGS => "Settings",
                    _ => "Quit",
                };
                let digit = u8::try_from(at + 1).unwrap_or(9);
                Button {
                    key: Some(Key::Digit(digit)),
                    state: if focus == Some(id) {
                        ButtonState::Selected
                    } else {
                        ButtonState::Enabled
                    },
                    ..Button::row(id.0, plain(label))
                }
            })
            .collect();
        Screen {
            title: Some(plain("SLAY THE SPIRE 2")),
            blocks: vec![
                Block::Text(text),
                Block::List {
                    frame: true,
                    rows,
                    page: None,
                },
            ],
            ..Screen::default()
        }
    }

    fn about(&self, describer: &Describer) -> Vec<Rich> {
        let character = self.character();
        let (hp, max_hp, gold) = start_stats(character, self.ascension);
        let relic = character.starting_relic();
        let mut about = vec![Rich(vec![
            RichSpan {
                fx: None,
                role: Role::Title,
                bold: true,
                text: character.name().to_uppercase(),
            },
            plain_span("  "),
            RichSpan {
                fx: None,
                role: Role::Hp,
                bold: false,
                text: format!("♥ {hp}/{max_hp}"),
            },
            plain_span("  "),
            RichSpan {
                fx: None,
                role: Role::Gold,
                bold: false,
                text: format!("◉ {gold}"),
            },
        ])];
        let mut relic_line = vec![RichSpan {
            fx: None,
            role: Role::Plain,
            bold: true,
            text: format!("{}: ", relic.name()),
        }];
        if let Some(markup) = describer.relic(relic.id()) {
            relic_line.extend(text::inline(&markup, Role::Body).0);
        }
        about.push(Rich(relic_line));
        if let Some(markup) = describer.character_description(character.name()) {
            about.extend(text::lines(&markup, Role::Body));
        }
        about
    }

    fn ascension_and_seed(&self, describer: &Describer) -> Vec<Button> {
        let level = self.ascension;
        let mut ascension = Button {
            key: Some(Key::Char('-')),
            right: Some(role(Role::Key, "+")),
            info: (level > 0).then(|| InfoRef::Room(format!("ascension:{level}"))),
            ..Button::row(
                ASCENSION.0,
                Rich(vec![
                    plain_span("Ascension "),
                    RichSpan {
                        fx: None,
                        role: Role::Key,
                        bold: true,
                        text: level.to_string(),
                    },
                ]),
            )
        };
        if level > 0 {
            let title = describer
                .ascension_title(level)
                .unwrap_or_else(|| format!("Ascension {level}"));
            let mut line = vec![RichSpan {
                fx: None,
                role: Role::Muted,
                bold: false,
                text: title,
            }];
            if let Some(markup) = describer.ascension_description(level) {
                line.push(RichSpan {
                    fx: None,
                    role: Role::Muted,
                    bold: false,
                    text: format!(": {}", text::plain(&markup)),
                });
            }
            ascension.detail = vec![Rich(line)];
        }
        let seed_text = if self.editing {
            format!("{}▌", self.seed)
        } else if self.seed.is_empty() {
            "random".to_owned()
        } else {
            self.seed.clone()
        };
        let seed = Button {
            key: Some(Key::Char('s')),
            state: if self.editing {
                ButtonState::Selected
            } else {
                ButtonState::Enabled
            },
            right: Some(role(Role::Plain, &seed_text)),
            ..Button::row(SEED.0, role(Role::Muted, "Seed"))
        };
        vec![ascension, seed]
    }

    fn select_screen(&self, describer: &Describer) -> Screen {
        let rows = SELECT_ORDER
            .iter()
            .enumerate()
            .map(|(at, &c)| {
                let id = u32::try_from(at + 1).unwrap_or(1);
                Button {
                    key: Some(Key::Digit(u8::try_from(at + 1).unwrap_or(1))),
                    right: Some(role(Role::Hp, &format!("{} HP", c.starting_hp()))),
                    state: if at == self.character {
                        ButtonState::Selected
                    } else {
                        ButtonState::Enabled
                    },
                    info: Some(InfoRef::Room(format!("character:{}", c.name()))),
                    ..Button::row(id, plain(c.name()))
                }
            })
            .collect();
        let mut about = self.about(describer);
        if self.saved.is_none()
            && let Some(note) = &self.crash
        {
            about.insert(
                0,
                role(
                    Role::Attack,
                    &format!("The game crashed last time ({note}). No run was saved."),
                ),
            );
        }
        let rows2 = self.ascension_and_seed(describer);
        let (label, key) = if self.editing {
            ("Done", Key::Enter)
        } else {
            ("Start", Key::Enter)
        };
        Screen {
            title: Some(plain("NEW RUN")),
            blocks: vec![
                Block::List {
                    frame: true,
                    rows,
                    page: None,
                },
                Block::Text(about),
                Block::List {
                    frame: true,
                    rows: rows2,
                    page: None,
                },
            ],
            primary: Some(Button {
                key: Some(key),
                ..Button::row(0, plain(label))
            }),
            ..Screen::default()
        }
    }
}

/// A character's HP at the start of a run, its max HP and its gold.
///
/// From the engine: `Run::new` starts at `starting_hp` and `starting_gold`, and Neow's
/// `BeforeEventStarted` sets the HP (`WearyTraveler` heals only 80% of it).
#[must_use]
pub fn start_stats(character: Character, ascension: u8) -> (i32, i32, i32) {
    let max_hp = character.starting_hp();
    (
        neow::hp_after_neow(max_hp, ascension),
        max_hp,
        character.starting_gold(),
    )
}

/// One settings row: its name, its value now and what it does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SettingRow {
    pub label: String,
    pub value: String,
    pub help: String,
}

/// The settings page: a row per setting whose value cycles on tap (`ButtonId(i)` is row `i`),
/// the selected row's help under them, then `notes` (the boss keys, the file).
#[must_use]
pub fn settings_screen(rows: &[SettingRow], selected: usize, notes: &[String]) -> Screen {
    let list = rows
        .iter()
        .enumerate()
        .map(|(i, row)| Button {
            right: Some(role(Role::Key, &format!("◀ {} ▶", row.value))),
            state: if i == selected {
                ButtonState::Selected
            } else {
                ButtonState::Enabled
            },
            ..Button::row(u32::try_from(i).unwrap_or(0), plain(&row.label))
        })
        .collect();
    let mut text = vec![role(
        Role::Muted,
        rows.get(selected).map_or("", |r| r.help.as_str()),
    )];
    text.push(Rich::default());
    text.extend(notes.iter().map(|n| role(Role::Muted, n)));
    Screen {
        title: Some(plain("Settings")),
        blocks: vec![
            Block::List {
                frame: true,
                rows: list,
                page: None,
            },
            Block::Text(text),
        ],
        ..Screen::default()
    }
}

fn plain_span(text: &str) -> RichSpan {
    RichSpan {
        fx: None,
        role: Role::Plain,
        bold: false,
        text: text.to_owned(),
    }
}

fn role(role: Role, text: &str) -> Rich {
    Rich(vec![RichSpan {
        fx: None,
        role,
        bold: false,
        text: text.to_owned(),
    }])
}

fn plain(text: impl Into<String>) -> Rich {
    Rich(vec![RichSpan {
        fx: None,
        role: Role::Plain,
        bold: false,
        text: text.into(),
    }])
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;
    use crate::text::Localization;

    fn describer() -> Describer {
        Describer::new(Rc::new(Localization::empty()))
    }

    #[test]
    fn keys_choose_the_character_the_ascension_and_the_seed() {
        let mut launch = Launch::new(Character::Ironclad, 0);
        launch.key(Key::Digit(3));
        assert_eq!(launch.character(), Character::Regent);
        launch.key(Key::Left);
        launch.key(Key::Left);
        launch.key(Key::Left);
        assert_eq!(launch.character(), SELECT_ORDER[4], "wraps");
        for _ in 0..20 {
            launch.key(Key::Up);
        }
        assert_eq!(launch.ascension(), MAX_ASCENSION);
        launch.key(Key::Char('s'));
        for c in ['a', 'b', '1'] {
            launch.key(Key::Char(c));
        }
        launch.key(Key::Backspace);
        assert_eq!(launch.seed(), "AB");
        launch.key(Key::Enter);
        assert!(!launch.is_editing_seed());
    }

    #[test]
    fn the_screens_print_the_keys_that_work() {
        let launch = Launch::new(Character::Silent, 1);
        let screen = launch.screen(&describer());
        let Block::List { rows, .. } = &screen.blocks[0] else {
            panic!("the characters");
        };
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[1].state, ButtonState::Selected);
        assert_eq!(
            screen.primary.as_ref().map(|b| b.key),
            Some(Some(Key::Enter))
        );
    }

    #[test]
    fn the_main_menu_walks_with_the_arrows_and_new_run_leaves_it() {
        let saved = SavedRun {
            who: "Silent · Ascension 2".to_owned(),
            place: "Act 1 · Floor 3".to_owned(),
        };
        let mut launch = Launch::new(Character::Ironclad, 0).with_save(Some(saved), None, false);
        assert_eq!(launch.focused(), Some(CONTINUE));
        launch.key(Key::Down);
        launch.key(Key::Down);
        assert_eq!(launch.focused(), Some(SETTINGS));
        launch.key(Key::Enter);
        assert_eq!(launch.take_request(), Some(Request::Settings));
        launch.key(Key::Up);
        launch.key(Key::Enter);
        assert!(!launch.menu_shown());
        assert_eq!(launch.focused(), None);
    }
}
