//! The character select wrapper: [`Launch`] with the saved run and the crash note the terminal
//! holds, and the key commands and clicks that drive it.

use std::path::PathBuf;
use std::rc::Rc;

use ratatui::crossterm::event::KeyCode;
use sts2_core::Character;
use sts2_view::launch::{Launch, SELECT_ORDER, SavedRun};
use sts2_view::model::{ButtonId, Input as ViewInput, Key as ViewKey, Screen as ViewScreen};

use crate::describe::Describer;
use crate::hit::{Target, UiAction};
use crate::keys::{self, Command, Layer};
use crate::session::Save;
use crate::text::Localization;

/// The character select screen's state: [`Launch`] with the saved run and the crash note the
/// terminal holds.
#[derive(Clone, Debug)]
pub struct CharacterSelect {
    pub(super) launch: Launch,
    describer: Describer,
    /// The run in progress the save file holds, offered as Continue.
    pub(super) saved: Option<Save>,
    /// The crash note the last session left, if it crashed.
    crash_note: Option<PathBuf>,
}

impl CharacterSelect {
    /// The select screen, with `character` highlighted.
    #[must_use]
    pub fn new(character: Character, ascension: u8, text: Rc<Localization>) -> Self {
        Self {
            launch: Launch::new(character, ascension),
            describer: Describer::new(text),
            saved: None,
            crash_note: None,
        }
    }

    /// Offers `save` as Continue (focused, so Enter continues), noting `crash_note` if the
    /// last session crashed.
    #[must_use]
    pub fn with_save(mut self, save: Option<Save>, crash_note: Option<PathBuf>) -> Self {
        let summary = save.as_ref().map(|s| SavedRun {
            who: format!("{} · Ascension {}", s.character.name(), s.ascension),
            place: format!(
                "Act {} · Floor {} · ♥ {}/{} · ◉ {}",
                s.act, s.floor, s.hp, s.max_hp, s.gold
            ),
        });
        let earlier = crash_note.is_some() && save.as_ref().and_then(Save::earlier).is_some();
        let note = crash_note
            .as_ref()
            .map(|p| format!("note: {}", p.display()));
        self.launch = self.launch.with_save(summary, note, earlier);
        self.saved = save;
        self.crash_note = crash_note;
        self
    }

    /// The saved run offered as Continue.
    #[must_use]
    pub const fn saved(&self) -> Option<&Save> {
        self.saved.as_ref()
    }

    /// Whether the main menu shows, Continue first (Enter continues the saved run).
    #[must_use]
    pub const fn continue_focused(&self) -> bool {
        self.launch.menu_shown()
    }

    /// The main menu's entries in order: Continue, the earlier save after a crash, New run,
    /// Settings, Quit.
    #[must_use]
    pub fn main_menu(&self) -> Vec<UiAction> {
        let mut entries = vec![UiAction::Continue];
        if self.can_continue_earlier() {
            entries.push(UiAction::ContinueEarlier);
        }
        entries.extend([UiAction::NewRun, UiAction::Settings, UiAction::Quit]);
        entries
    }

    /// What the keyboard focus rests on: the main menu's entry, while it shows.
    #[must_use]
    pub fn focused(&self) -> Option<Target> {
        self.launch
            .focused()
            .and_then(menu_action)
            .map(Target::Button)
    }

    /// Where the last session's crash note is, if it crashed.
    #[must_use]
    pub fn crash_note(&self) -> Option<&std::path::Path> {
        self.crash_note.as_deref()
    }

    /// Whether the saved run is offered from before its last action too (after a crash).
    #[must_use]
    pub const fn can_continue_earlier(&self) -> bool {
        self.launch.can_continue_earlier()
    }

    /// The key bindings that apply now, the first match winning.
    #[must_use]
    pub fn layers(&self) -> Vec<Layer> {
        if self.is_editing_seed() {
            return vec![keys::SEED];
        }
        let mut layers = Vec::new();
        if self.continue_focused() {
            layers.push(keys::CONTINUE);
        }
        if self.can_continue_earlier() {
            layers.push(keys::CONTINUE_EARLIER);
        }
        layers.push(keys::SELECT);
        layers
    }

    #[must_use]
    pub const fn character(&self) -> Character {
        self.launch.character()
    }

    #[must_use]
    pub const fn ascension(&self) -> u8 {
        self.launch.ascension()
    }

    /// The custom seed typed so far (empty: a random seed is rolled at the start).
    #[must_use]
    pub fn seed(&self) -> &str {
        self.launch.seed()
    }

    #[must_use]
    pub const fn is_editing_seed(&self) -> bool {
        self.launch.is_editing_seed()
    }

    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.launch.error()
    }

    /// The game text, for the character and ascension blurbs.
    #[must_use]
    pub const fn describer(&self) -> &Describer {
        &self.describer
    }

    /// The screen the select shows.
    #[must_use]
    pub fn screen(&self) -> ViewScreen {
        self.launch.screen(&self.describer)
    }

    /// Carries out a click's (or a key's) action.
    pub(super) fn perform(&mut self, action: UiAction) {
        // Choosing anything for a new run leaves the main menu (the settings do not).
        let on_menu = self.continue_focused();
        if on_menu
            && !matches!(
                action,
                UiAction::Continue
                    | UiAction::ContinueEarlier
                    | UiAction::NewRun
                    | UiAction::Quit
                    | UiAction::Settings
                    | UiAction::Back
            )
        {
            self.launch
                .input(&ViewInput::Tap(sts2_view::launch::NEW_RUN));
        }
        let input = match action {
            UiAction::Continue if on_menu => ViewInput::Tap(sts2_view::launch::CONTINUE),
            UiAction::ContinueEarlier if on_menu => ViewInput::Tap(sts2_view::launch::EARLIER),
            UiAction::NewRun if on_menu => ViewInput::Tap(sts2_view::launch::NEW_RUN),
            UiAction::Settings if on_menu => ViewInput::Tap(sts2_view::launch::SETTINGS),
            UiAction::Settings => {
                self.launch.request_settings();
                return;
            }
            UiAction::Quit => ViewInput::Key(ViewKey::Char('q')),
            UiAction::SelectCharacter(i) if i < SELECT_ORDER.len() => {
                ViewInput::Tap(ButtonId(u32::try_from(i + 1).unwrap_or(1)))
            }
            UiAction::Ascension(up) => ViewInput::Tap(if up {
                sts2_view::launch::ASCENSION_UP
            } else {
                sts2_view::launch::ASCENSION_DOWN
            }),
            UiAction::EditSeed => ViewInput::Tap(sts2_view::launch::SEED),
            UiAction::RandomSeed => ViewInput::Key(ViewKey::Char('r')),
            UiAction::Start => ViewInput::Primary,
            UiAction::Back => ViewInput::Back,
            _ => return,
        };
        self.launch.input(&input);
    }

    /// Carries out what a key's binding says.
    pub(super) fn command(&mut self, command: Command, key: KeyCode) {
        let view = match (command, key) {
            (Command::Ui(action), _) => {
                self.perform(action);
                return;
            }
            (Command::Move(by), _) => {
                if self.continue_focused() {
                    self.launch
                        .input(&ViewInput::Tap(sts2_view::launch::NEW_RUN));
                }
                if by < 0 {
                    ViewKey::Left
                } else {
                    ViewKey::Right
                }
            }
            (Command::Focus(by), _) => {
                if by < 0 {
                    ViewKey::Up
                } else {
                    ViewKey::Down
                }
            }
            (Command::Activate, _) => ViewKey::Enter,
            (Command::Digit, KeyCode::Char(c @ '1'..='9')) => ViewKey::Digit(c as u8 - b'0'),
            (Command::Type, KeyCode::Char(c)) => ViewKey::Char(c),
            (Command::Erase, _) => ViewKey::Backspace,
            _ => return,
        };
        self.launch.key(view);
    }
}

/// The action of a main menu row.
const fn menu_action(id: ButtonId) -> Option<UiAction> {
    Some(match id {
        sts2_view::launch::CONTINUE => UiAction::Continue,
        sts2_view::launch::EARLIER => UiAction::ContinueEarlier,
        sts2_view::launch::NEW_RUN => UiAction::NewRun,
        sts2_view::launch::SETTINGS => UiAction::Settings,
        sts2_view::launch::QUIT => UiAction::Quit,
        _ => return None,
    })
}
