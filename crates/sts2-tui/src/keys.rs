//! The global key table: one table per screen state, read by the input handlers to decide what a
//! key does and by the help overlay (`?`) to list them, so the two cannot drift apart.
//!
//! A screen's keys are a stack of [`Layer`]s, the first match winning: the run's are built
//! by [`App::layers`](crate::app::App::layers), the select screen's by
//! [`CharacterSelect::layers`](crate::shell::CharacterSelect::layers), and the ones that work
//! everywhere by [`shell_layer`]. The shell's `input` module is the only reader of the stack
//! besides the help. The arrows and Enter ([`NAV`]) move and press a highlighted
//! choice on every screen; the numbers and letters stay as shortcuts.

use ratatui::crossterm::event::KeyCode;

use crate::app::Overlay;
use sts2_view::combat::POTION_KEYS;

use crate::hit::UiAction;

/// The keys a binding answers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keys {
    /// These keys.
    Codes(&'static [KeyCode]),
    /// Any digit.
    Digits,
    /// A letter or a digit (typing a seed).
    Typing,
    /// The boss keys of the settings (`` ` `` and F12 by default).
    Boss,
    /// Any key the layers above did not take.
    Other,
}

impl Keys {
    /// Whether `key` is one of these; `boss` lists the boss keys.
    #[must_use]
    pub fn matches(self, key: KeyCode, boss: &[KeyCode]) -> bool {
        match self {
            Self::Codes(codes) => codes.contains(&key),
            Self::Digits => matches!(key, KeyCode::Char(c) if c.is_ascii_digit()),
            Self::Typing => matches!(key, KeyCode::Char(c) if c.is_ascii_alphanumeric()),
            Self::Boss => boss.contains(&key),
            Self::Other => true,
        }
    }
}

/// What a key does, before the screen it was pressed on carries it out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// The same action a click makes.
    Ui(UiAction),
    /// A button that cannot be undone was pressed and waits: its own key again confirms, any
    /// other key puts its label back.
    Confirm,
    /// Types a digit of the number being picked.
    Digit,
    /// Moves the keyboard focus (the highlighted choice) back or on by this many; with nothing
    /// to focus it scrolls the open list.
    Focus(isize),
    /// Presses the focused choice (or picks the number typed so far).
    Activate,
    /// Erases the last digit, or the seed's last character.
    Erase,
    /// Types a character of the seed.
    Type,
    /// Moves to the previous or next character on the select screen, or in the settings list.
    Move(isize),
    /// Settings: the previous or next value of the selected row.
    Cycle(bool),
    /// Opens the help, the settings or the boss screen.
    Help,
    Settings,
    Boss,
    /// Closes the help or the settings.
    Close,
    /// Does nothing (the key is taken so nothing below sees it).
    Ignore,
    /// Says the key does nothing here.
    Unknown,
    /// In the inspect overlay: a digit picks what to inspect (a card, an option; an enemy after
    /// `e`), `e` switches the next digit to the enemies.
    Inspect,
    InspectEnemy,
    /// The card reward's Skip row (`s`, printed on it).
    SkipCard,
}

/// One row of a table: the keys, how the help writes them, what they do in words and in fact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    pub keys: Keys,
    /// The keys as the help shows them (`←/→`, `1-9`).
    pub label: &'static str,
    pub help: &'static str,
    pub command: Command,
}

const fn with_keys(
    keys: Keys,
    label: &'static str,
    help: &'static str,
    command: Command,
) -> Binding {
    Binding {
        keys,
        label,
        help,
        command,
    }
}

const fn bind(
    keys: &'static [KeyCode],
    label: &'static str,
    help: &'static str,
    command: Command,
) -> Binding {
    with_keys(Keys::Codes(keys), label, help, command)
}

const fn ui(
    keys: &'static [KeyCode],
    label: &'static str,
    help: &'static str,
    action: UiAction,
) -> Binding {
    bind(keys, label, help, Command::Ui(action))
}

/// A titled group of bindings that applies in one state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layer {
    pub title: &'static str,
    pub bindings: &'static [Binding],
}

/// The first binding of `layers` that takes `key`.
#[must_use]
pub fn lookup(layers: &[Layer], key: KeyCode, boss: &[KeyCode]) -> Option<(Layer, Binding)> {
    layers.iter().find_map(|layer| {
        layer
            .bindings
            .iter()
            .find(|b| b.keys.matches(key, boss))
            .map(|b| (*layer, *b))
    })
}

const fn ch(c: char) -> KeyCode {
    KeyCode::Char(c)
}

/// The keys that work on every screen.
pub const SHELL: Layer = Layer {
    title: "Anywhere",
    bindings: &[
        with_keys(
            Keys::Boss,
            "boss key",
            "hide the game behind a work-safe screen (any key comes back)",
            Command::Boss,
        ),
        bind(&[ch('?')], "?", "this help", Command::Help),
        bind(&[ch('o')], "o", "settings", Command::Settings),
    ],
};

/// While a seed is typed only the boss key works everywhere: the rest are letters.
pub const SHELL_TYPING: Layer = Layer {
    title: "Anywhere",
    bindings: &[with_keys(
        Keys::Boss,
        "boss key",
        "hide the game behind a work-safe screen (any key comes back)",
        Command::Boss,
    )],
};

/// The shell's layer for the state: [`SHELL_TYPING`] while typing, else [`SHELL`].
#[must_use]
pub const fn shell_layer(typing: bool) -> Layer {
    if typing { SHELL_TYPING } else { SHELL }
}

/// The keyboard focus: a highlighted choice on every screen, moved by the arrows, pressed by Enter.
///
/// Where choices stand in a row (the next rooms on the map, the targets of a card) the left and
/// right arrows move it too.
pub const NAV: Layer = Layer {
    title: "Highlight",
    bindings: &[
        bind(
            &[KeyCode::Up],
            "↑",
            "move the highlight up (an open list: scroll)",
            Command::Focus(-1),
        ),
        bind(
            &[KeyCode::Down],
            "↓",
            "move the highlight down",
            Command::Focus(1),
        ),
        bind(
            &[KeyCode::Left],
            "←",
            "move it back: the previous room or target",
            Command::Focus(-1),
        ),
        bind(
            &[KeyCode::Right],
            "→",
            "move it on: the next room or target",
            Command::Focus(1),
        ),
        bind(
            &[KeyCode::Enter, ch(' ')],
            "Enter/Space",
            "press the highlighted choice: play the card, pick the option, travel",
            Command::Activate,
        ),
    ],
};

/// The help overlay: the arrows scroll it, any other key closes it.
pub const HELP: Layer = Layer {
    title: "Help",
    bindings: &[
        bind(
            &[KeyCode::Up, ch('k')],
            "↑/k",
            "scroll up",
            Command::Move(-1),
        ),
        bind(
            &[KeyCode::Down, ch('j')],
            "↓/j",
            "scroll down",
            Command::Move(1),
        ),
        bind(&[KeyCode::PageUp], "PgUp", "a page up", Command::Move(-10)),
        bind(
            &[KeyCode::PageDown],
            "PgDn",
            "a page down",
            Command::Move(10),
        ),
        with_keys(
            Keys::Other,
            "any other key",
            "close the help",
            Command::Close,
        ),
    ],
};

pub const SETTINGS: Layer = Layer {
    title: "Settings",
    bindings: &[
        bind(
            &[KeyCode::Up, ch('k')],
            "↑/k",
            "previous setting",
            Command::Move(-1),
        ),
        bind(
            &[KeyCode::Down, ch('j')],
            "↓/j",
            "next setting",
            Command::Move(1),
        ),
        bind(
            &[KeyCode::Left, ch('h'), ch('-')],
            "←/h",
            "previous value",
            Command::Cycle(false),
        ),
        bind(
            &[KeyCode::Right, ch('l'), ch('+'), KeyCode::Enter, ch(' ')],
            "→/l/Enter",
            "next value (saved at once)",
            Command::Cycle(true),
        ),
        bind(
            &[KeyCode::Esc, ch('o'), ch('q')],
            "Esc/o/q",
            "close the settings",
            Command::Close,
        ),
        with_keys(Keys::Other, "", "", Command::Ignore),
    ],
};

/// The main menu while a saved run waits: Continue is highlighted first.
pub const CONTINUE: Layer = Layer {
    title: "Saved run",
    bindings: &[
        bind(
            &[KeyCode::Enter, ch(' ')],
            "Enter",
            "the highlighted entry (Continue at first)",
            Command::Activate,
        ),
        bind(
            &[KeyCode::Up],
            "↑",
            "highlight the entry above",
            Command::Focus(-1),
        ),
        bind(
            &[KeyCode::Down],
            "↓",
            "highlight the entry below",
            Command::Focus(1),
        ),
        // The menu is one column: the side arrows walk it too, rather than falling through
        // to the character select's and leaving the menu.
        bind(
            &[KeyCode::Left],
            "←",
            "highlight the entry above",
            Command::Focus(-1),
        ),
        bind(
            &[KeyCode::Right],
            "→",
            "highlight the entry below",
            Command::Focus(1),
        ),
        ui(
            &[ch('c')],
            "c",
            "continue the saved run",
            UiAction::Continue,
        ),
        ui(
            &[ch('n')],
            "n",
            "set up a new run instead (the save goes when it starts)",
            UiAction::NewRun,
        ),
    ],
};

/// After a crash: the saved run can also be continued from one action earlier.
pub const CONTINUE_EARLIER: Layer = Layer {
    title: "Saved run",
    bindings: &[ui(
        &[ch('b')],
        "b",
        "continue from before the last action (if it crashed the game)",
        UiAction::ContinueEarlier,
    )],
};

pub const SELECT: Layer = Layer {
    title: "Character select",
    bindings: &[
        bind(
            &[KeyCode::Left, ch('h')],
            "←/h",
            "previous character",
            Command::Move(-1),
        ),
        bind(
            &[KeyCode::Right, ch('l')],
            "→/l",
            "next character",
            Command::Move(1),
        ),
        ui(
            &[KeyCode::Up, ch('+'), ch('k')],
            "↑/+/k",
            "ascension up",
            UiAction::Ascension(true),
        ),
        ui(
            &[KeyCode::Down, ch('-'), ch('j')],
            "↓/-/j",
            "ascension down",
            UiAction::Ascension(false),
        ),
        with_keys(
            Keys::Codes(&[ch('1'), ch('2'), ch('3'), ch('4'), ch('5')]),
            "1-5",
            "pick the character by its number",
            Command::Digit,
        ),
        ui(&[ch('s')], "s", "type a custom seed", UiAction::EditSeed),
        ui(&[ch('r')], "r", "random seed", UiAction::RandomSeed),
        ui(
            &[KeyCode::Enter, ch(' ')],
            "Enter",
            "start the run",
            UiAction::Start,
        ),
        ui(&[ch('q'), KeyCode::Esc], "q/Esc", "quit", UiAction::Quit),
    ],
};

pub const SEED: Layer = Layer {
    title: "Typing a seed",
    bindings: &[
        with_keys(
            Keys::Typing,
            "letters, digits",
            "type the seed",
            Command::Type,
        ),
        bind(&[KeyCode::Backspace], "Backspace", "erase", Command::Erase),
        ui(
            &[KeyCode::Enter, KeyCode::Esc],
            "Enter/Esc",
            "done",
            UiAction::EditSeed,
        ),
        with_keys(Keys::Other, "", "", Command::Ignore),
    ],
};

/// A button that cannot be undone (Leave with rewards left, Skip, Buy, Remove) was pressed
/// once and shows its question in place.
pub const CONFIRM: Layer = Layer {
    title: "Confirming",
    bindings: &[
        ui(
            &[KeyCode::Enter, ch(' ')],
            "Enter",
            "confirm the pressed button (its own key again does too)",
            UiAction::Yes,
        ),
        with_keys(
            Keys::Codes(&[KeyCode::Up, KeyCode::Down, KeyCode::Left, KeyCode::Right]),
            "",
            "",
            Command::Ignore,
        ),
        with_keys(
            Keys::Other,
            "any other key",
            "put the button back",
            Command::Confirm,
        ),
    ],
};

/// The inspect overlay (`i`): what a digit inspects.
pub const INSPECT: Layer = Layer {
    title: "Inspect",
    bindings: &[
        with_keys(
            Keys::Digits,
            "1-9, 0",
            "inspect that card or option (after e: that enemy)",
            Command::Inspect,
        ),
        bind(
            &[ch('e')],
            "e",
            "the next digit inspects an enemy",
            Command::InspectEnemy,
        ),
        ui(
            &[KeyCode::Esc, KeyCode::Backspace, ch('i')],
            "Esc/Backspace/i",
            "close it",
            UiAction::Back,
        ),
    ],
};

/// The card reward's screen: its Skip row's letter, printed on the row.
pub const CARD_REWARD: Layer = Layer {
    title: "Card reward",
    bindings: &[bind(
        &[ch('s')],
        "s",
        "skip the card reward",
        Command::SkipCard,
    )],
};

/// The `≡` menu: a potion is thrown away with `t` and its slot, as the menu prints them.
pub const MENU: Layer = Layer {
    title: "Menu",
    bindings: &[with_keys(
        Keys::Digits,
        "1-5",
        "throw away the potion in that belt slot, as its row prints it (asks in place)",
        Command::Digit,
    )],
};

/// The key of belt slot `slot` (0-based), `?` past the fifth.
#[must_use]
pub fn potion_key(slot: usize) -> char {
    POTION_KEYS.get(slot).copied().unwrap_or('?')
}

/// The run's keys that work on every screen of it.
pub const RUN: Layer = Layer {
    title: "In a run",
    bindings: &[
        ui(
            &[ch('q')],
            "q",
            "quit (the run is saved; Continue picks it up)",
            UiAction::Quit,
        ),
        ui(
            &[ch('w')],
            "w",
            "ask the bot what it would do here (what it sees, as you do)",
            UiAction::Hint,
        ),
        ui(
            &[KeyCode::PageUp],
            "PgUp",
            "scroll up a page; a long list: the previous page",
            UiAction::Scroll(-5),
        ),
        ui(
            &[KeyCode::PageDown],
            "PgDn",
            "scroll down a page; a long list: the next page",
            UiAction::Scroll(5),
        ),
        ui(&[ch('k')], "k", "scroll up", UiAction::Scroll(-1)),
        ui(&[ch('j')], "j", "scroll down", UiAction::Scroll(1)),
        ui(
            &[KeyCode::Tab],
            "Tab",
            "the menu (≡): deck, relics, potions, piles, map, log, rooms, help, settings, quit",
            UiAction::Toggle(Overlay::Menu),
        ),
        ui(&[ch('d')], "d", "deck", UiAction::Toggle(Overlay::Deck)),
        ui(&[ch('v')], "v", "relics", UiAction::Toggle(Overlay::Relics)),
        ui(&[ch('m')], "m", "map", UiAction::Toggle(Overlay::Map)),
        ui(
            &[ch('l')],
            "l",
            "log (fight only)",
            UiAction::Toggle(Overlay::Log),
        ),
        ui(
            &[ch('a')],
            "a",
            "draw pile (fight only, as the pile line prints it)",
            UiAction::Toggle(Overlay::DrawPile),
        ),
        ui(
            &[ch('s')],
            "s",
            "discard pile (fight only)",
            UiAction::Toggle(Overlay::DiscardPile),
        ),
        ui(
            &[ch('x')],
            "x",
            "exhaust pile (fight only)",
            UiAction::Toggle(Overlay::ExhaustPile),
        ),
        ui(
            &[ch('i')],
            "i",
            "potions and powers; then a number inspects a card or an option, e and a number \
             an enemy",
            UiAction::Toggle(Overlay::Info),
        ),
        ui(
            &[ch('h')],
            "h",
            "rooms visited",
            UiAction::Toggle(Overlay::History),
        ),
    ],
};

pub const OVERLAY: Layer = Layer {
    title: "Open list",
    bindings: &[
        ui(
            &[KeyCode::Esc, KeyCode::Backspace],
            "Esc/Backspace",
            "close it (or its key again)",
            UiAction::Back,
        ),
        with_keys(Keys::Other, "", "", Command::Ignore),
    ],
};

/// The end of the run: the buttons print their keys (`sts2_view::rooms::END_BUTTONS`), every
/// other key does nothing.
pub const END: Layer = Layer {
    title: "End of the run",
    bindings: &[with_keys(Keys::Other, "", "", Command::Ignore)],
};

pub const PICK: Layer = Layer {
    title: "Choosing",
    bindings: &[
        with_keys(
            Keys::Digits,
            "1-9, 0",
            "pick by number: a card, an option, a target (0 is the tenth; a longer list is \
             paged: PgUp/PgDn turn it and 0 is the way on, Leave or Done); the fight's other \
             keys are listed under Choosing",
            Command::Digit,
        ),
        bind(
            &[KeyCode::Backspace],
            "Backspace",
            "erase a digit, else cancel the selection",
            Command::Erase,
        ),
        ui(
            &[KeyCode::Esc],
            "Esc",
            "back: put the card back, close the card reward, cancel a card selection, leave \
             the shop; every other key is printed on its button",
            UiAction::Back,
        ),
        with_keys(Keys::Other, "", "", Command::Unknown),
    ],
};

/// Every layer, for the tests that check the help lists every key.
pub const ALL: [Layer; 17] = [
    SHELL,
    SHELL_TYPING,
    NAV,
    HELP,
    SETTINGS,
    CONTINUE,
    CONTINUE_EARLIER,
    SELECT,
    SEED,
    CONFIRM,
    INSPECT,
    CARD_REWARD,
    MENU,
    RUN,
    OVERLAY,
    END,
    PICK,
];

/// A mouse gesture and what it does, for the help.
pub type Gesture = (&'static str, &'static str);

/// What the mouse does on the select screen.
pub const MOUSE_SELECT: &[Gesture] = &[
    ("click a character", "choose it"),
    (
        "click ◀ ▶ or wheel over the ascension",
        "change the ascension",
    ),
    ("click a button", "do what it says"),
];

/// What the mouse does in a run.
pub const MOUSE_RUN: &[Gesture] = &[
    (
        "click a card",
        "play it (a card that needs a target then waits for a click on one)",
    ),
    ("click an enemy", "the target of the waiting card or potion"),
    ("drag a card", "onto an enemy or the battlefield to play it"),
    (
        "click a potion",
        "select it: used at once at its one target, else a click on an enemy aims it, or a \
         second click uses one that needs no target",
    ),
    ("click an option or a map node", "choose it"),
    (
        "click ≡",
        "the menu: deck, relics, potions, piles, map, log, rooms, help, settings, quit",
    ),
    ("right click", "back: cancel or close"),
    ("wheel", "scroll what is under the pointer"),
];

/// How the help writes a key in a settings file and back (`F12`, `` ` ``, `Esc`).
#[must_use]
pub fn key_name(key: KeyCode) -> String {
    match key {
        KeyCode::Char(' ') => "Space".to_owned(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::F(n) => format!("F{n}"),
        KeyCode::Esc => "Esc".to_owned(),
        KeyCode::Enter => "Enter".to_owned(),
        KeyCode::Tab => "Tab".to_owned(),
        KeyCode::Pause => "Pause".to_owned(),
        KeyCode::Insert => "Insert".to_owned(),
        KeyCode::ScrollLock => "ScrollLock".to_owned(),
        other => format!("{other:?}"),
    }
}

/// [`key_name`]'s inverse.
#[must_use]
pub fn parse_key(name: &str) -> Option<KeyCode> {
    let mut chars = name.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => return Some(KeyCode::Char(c)),
        (None, _) => return None,
        _ => {}
    }
    match name {
        "Space" => Some(KeyCode::Char(' ')),
        "Esc" => Some(KeyCode::Esc),
        "Enter" => Some(KeyCode::Enter),
        "Tab" => Some(KeyCode::Tab),
        "Pause" => Some(KeyCode::Pause),
        "Insert" => Some(KeyCode::Insert),
        "ScrollLock" => Some(KeyCode::ScrollLock),
        _ => name
            .strip_prefix('F')
            .and_then(|n| n.parse().ok())
            .filter(|n| (1..=24).contains(n))
            .map(KeyCode::F),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_names_round_trip() {
        for key in [
            ch('`'),
            KeyCode::F(12),
            ch(' '),
            KeyCode::Esc,
            ch('x'),
            KeyCode::Pause,
        ] {
            assert_eq!(parse_key(&key_name(key)), Some(key));
        }
        assert_eq!(parse_key("F99"), None);
        assert_eq!(parse_key(""), None);
    }

    #[test]
    fn the_first_layer_that_takes_a_key_wins() {
        let layers = [CONFIRM, NAV, RUN, PICK];
        let (layer, binding) = lookup(&layers, KeyCode::Enter, &[]).expect("bound");
        assert_eq!(layer.title, CONFIRM.title);
        assert_eq!(binding.command, Command::Ui(UiAction::Yes));
        let (layer, binding) = lookup(&layers[1..], KeyCode::Down, &[]).expect("bound");
        assert_eq!(layer.title, NAV.title);
        assert_eq!(binding.command, Command::Focus(1));
        let (_, binding) = lookup(&layers[1..], ch('y'), &[]).expect("the fallback");
        assert_eq!(binding.command, Command::Unknown);
    }
}
