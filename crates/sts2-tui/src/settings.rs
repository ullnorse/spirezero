//! The player's settings, kept in `settings.toml` in the config directory
//! ([`crate::session::Dirs`]) and saved as soon as one changes in the game (`o`).
//!
//! The file is flat TOML (`key = value`, `#` comments) and is read leniently: an unknown key is
//! ignored and a bad value falls back to its default, so an old or hand-edited file never stops
//! the game from starting.

use std::io;
use std::path::Path;

use ratatui::crossterm::event::KeyCode;
use sts2_core::Character;
use sts2_core::run::ascension::MAX_ASCENSION;

use crate::keys;

/// The colours the screens are drawn in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Theme {
    /// The colours as drawn, for a dark terminal.
    #[default]
    Dark,
    /// Dark text for a light terminal.
    Light,
    /// Greys only.
    Muted,
}

/// The glyph set the screens are drawn with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Glyphs {
    /// The glyphs as drawn (`ϟ ★ ◈ ◆ ◉ ⚔ ▣ ▲ ▼ █ ▒ ░`).
    #[default]
    Unicode,
    /// ASCII stand-ins, for a font without them (`E * o @ $ ! # + - = : .`).
    Ascii,
}

/// What the boss key shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BossView {
    /// A compiler's build output.
    #[default]
    Build,
    /// A `top`-like process list.
    Top,
}

/// Names a setting's values in the file and on the settings screen.
trait Named: Sized + Copy + PartialEq + 'static {
    const ALL: &'static [Self];
    fn name(self) -> &'static str;

    fn parse(text: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|v| v.name() == text)
    }

    /// The next (or previous) value, wrapping.
    fn cycle(self, forward: bool) -> Self {
        let n = Self::ALL.len();
        let i = Self::ALL.iter().position(|&v| v == self).unwrap_or(0);
        Self::ALL[if forward {
            (i + 1) % n
        } else {
            (i + n - 1) % n
        }]
    }
}

impl Named for Theme {
    const ALL: &'static [Self] = &[Self::Dark, Self::Light, Self::Muted];
    fn name(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::Muted => "muted",
        }
    }
}

impl Named for Glyphs {
    const ALL: &'static [Self] = &[Self::Unicode, Self::Ascii];
    fn name(self) -> &'static str {
        match self {
            Self::Unicode => "unicode",
            Self::Ascii => "ascii",
        }
    }
}

impl Theme {
    /// The theme named `text` in the file or on the command line (`dark`, `light`, `muted`).
    #[must_use]
    pub fn from_name(text: &str) -> Option<Self> {
        Named::parse(text)
    }

    /// The theme a terminal's `COLORFGBG` (`15;0` dark, `0;15` light: the last field is the
    /// background's colour number) asks for; `None` when the value says nothing.
    #[must_use]
    pub fn from_colorfgbg(value: &str) -> Option<Self> {
        let background: u8 = value.rsplit(';').next()?.trim().parse().ok()?;
        Some(match background {
            7 | 9..=15 => Self::Light,
            _ => Self::Dark,
        })
    }
}

impl Named for BossView {
    const ALL: &'static [Self] = &[Self::Build, Self::Top];
    fn name(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Top => "top",
        }
    }
}

/// Everything the player can set.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent on/off switches, each a row of the settings screen and a key of the file"
)]
pub struct Settings {
    /// Whether the game takes the mouse (`--no-mouse` overrides it for one session).
    pub mouse: bool,
    pub theme: Theme,
    /// Muted colours, whatever the theme says.
    pub discreet: bool,
    pub glyphs: Glyphs,
    /// Whether bars drain and hits float for a moment after a play (off: every frame rests).
    pub motion: bool,
    /// Whether menu entries that throw something away (leaving rewards behind) ask first.
    pub confirmations: bool,
    pub boss_view: BossView,
    pub boss_keys: Vec<KeyCode>,
    /// The select screen's choices when it last started a run.
    pub last_character: Character,
    pub last_ascension: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mouse: true,
            theme: Theme::Dark,
            discreet: false,
            glyphs: Glyphs::Unicode,
            motion: true,
            confirmations: true,
            boss_view: BossView::Build,
            boss_keys: vec![KeyCode::Char('`'), KeyCode::F(12)],
            last_character: crate::shell::SELECT_ORDER[0],
            last_ascension: 0,
        }
    }
}

/// One row of the settings screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Mouse,
    Theme,
    Discreet,
    Glyphs,
    Motion,
    Confirmations,
    BossView,
}

impl Row {
    /// The rows, in the screen's order.
    pub const ALL: [Self; 7] = [
        Self::Discreet,
        Self::Theme,
        Self::Glyphs,
        Self::Mouse,
        Self::Confirmations,
        Self::BossView,
        Self::Motion,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Mouse => "Mouse",
            Self::Theme => "Colours",
            Self::Discreet => "Discreet mode",
            Self::Glyphs => "Glyphs",
            Self::Motion => "Animations",
            Self::Confirmations => "Ask before leaving rewards",
            Self::BossView => "Boss key shows",
        }
    }

    /// What the row is for, in a few words.
    #[must_use]
    pub const fn help(self) -> &'static str {
        match self {
            Self::Mouse => "off leaves the mouse to the terminal (to select text)",
            Self::Theme => "dark or light terminal, or greys only",
            Self::Discreet => "greys only: looks like work",
            Self::Glyphs => "ASCII when the font lacks the glyphs (ϟ ★ ◆ ⚔ █)",
            Self::Motion => "bars drain and hits float for a moment; off: no motion",
            Self::Confirmations => "a second key press before throwing a reward away",
            Self::BossView => "a build log or a process list, no colours",
        }
    }
}

const fn on_off(on: bool) -> &'static str {
    if on { "on" } else { "off" }
}

impl Settings {
    /// The value of `row`, as the settings screen shows it.
    #[must_use]
    pub fn value(&self, row: Row) -> &'static str {
        match row {
            Row::Mouse => on_off(self.mouse),
            Row::Theme => self.theme.name(),
            Row::Discreet => on_off(self.discreet),
            Row::Glyphs => self.glyphs.name(),
            Row::Motion => on_off(self.motion),
            Row::Confirmations => on_off(self.confirmations),
            Row::BossView => match self.boss_view {
                BossView::Build => "build log",
                BossView::Top => "process list",
            },
        }
    }

    /// Moves `row` to its next (or previous) value.
    pub fn cycle(&mut self, row: Row, forward: bool) {
        match row {
            Row::Mouse => self.mouse = !self.mouse,
            Row::Theme => self.theme = self.theme.cycle(forward),
            Row::Discreet => self.discreet = !self.discreet,
            Row::Glyphs => self.glyphs = self.glyphs.cycle(forward),
            Row::Motion => self.motion = !self.motion,
            Row::Confirmations => self.confirmations = !self.confirmations,
            Row::BossView => self.boss_view = self.boss_view.cycle(forward),
        }
    }

    /// The colours to draw in.
    #[must_use]
    pub const fn palette(&self) -> Theme {
        if self.discreet {
            Theme::Muted
        } else {
            self.theme
        }
    }

    /// The boss keys as the help writes them: `` ` / F12 ``.
    #[must_use]
    pub fn boss_key_names(&self) -> String {
        self.boss_keys
            .iter()
            .map(|&k| keys::key_name(k))
            .collect::<Vec<_>>()
            .join(" / ")
    }

    /// The settings in `path`; the defaults where it has none (or no file).
    #[must_use]
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .map(|text| Self::parse(&text))
            .unwrap_or_default()
    }

    /// Reads a settings file, keeping the default of every key it lacks or cannot read.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut s = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = Value::parse(value);
            match (key.trim(), value) {
                ("mouse", Some(Value::Bool(v))) => s.mouse = v,
                ("theme", Some(Value::Str(v))) => s.theme = Named::parse(&v).unwrap_or(s.theme),
                ("discreet", Some(Value::Bool(v))) => s.discreet = v,
                ("glyphs", Some(Value::Str(v))) => {
                    s.glyphs = Named::parse(&v).unwrap_or(s.glyphs);
                }
                ("motion", Some(Value::Bool(v))) => s.motion = v,
                ("confirmations", Some(Value::Bool(v))) => s.confirmations = v,
                ("boss_view", Some(Value::Str(v))) => {
                    s.boss_view = Named::parse(&v).unwrap_or(s.boss_view);
                }
                ("boss_keys", Some(Value::List(keys))) => {
                    let keys: Vec<KeyCode> =
                        keys.iter().filter_map(|k| keys::parse_key(k)).collect();
                    if !keys.is_empty() {
                        s.boss_keys = keys;
                    }
                }
                ("last_character", Some(Value::Str(v))) => {
                    s.last_character = Character::from_name(&v).unwrap_or(s.last_character);
                }
                ("last_ascension", Some(Value::Int(v))) => {
                    s.last_ascension = u8::try_from(v).map_or(0, |a| a.min(MAX_ASCENSION));
                }
                _ => {}
            }
        }
        s
    }

    /// The settings as the file holds them.
    #[must_use]
    pub fn to_toml(&self) -> String {
        let keys: Vec<String> = self
            .boss_keys
            .iter()
            .map(|&k| quote(&keys::key_name(k)))
            .collect();
        format!(
            "# Slay the Spire 2 TUI settings. The game saves this file when a setting changes \
             (`o` in the game);\n# it can be edited by hand too. A bad or missing value means \
             the default.\n\n\
             # greys only, whatever the theme says\n\
             discreet = {}\n\
             # dark | light | muted\n\
             theme = {}\n\
             # unicode | ascii: the glyphs the screens use\n\
             glyphs = {}\n\
             # bars drain and hits float for a moment\n\
             motion = {}\n\
             mouse = {}\n\
             # ask before leaving rewards behind\n\
             confirmations = {}\n\
             # build | top: what the boss key shows\n\
             boss_view = {}\n\
             # keys that show it: a character, F1-F24, Esc, Tab, Pause, Insert, ScrollLock\n\
             boss_keys = [{}]\n\n\
             # the last run's choices on the character select\n\
             last_character = {}\n\
             last_ascension = {}\n",
            self.discreet,
            quote(self.theme.name()),
            quote(self.glyphs.name()),
            self.motion,
            self.mouse,
            self.confirmations,
            quote(self.boss_view.name()),
            keys.join(", "),
            quote(self.last_character.name()),
            self.last_ascension,
        )
    }

    /// Writes the settings to `path` (through a temporary file, so a crash never leaves half
    /// a file).
    ///
    /// # Errors
    /// If the directory cannot be made or the file written.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        crate::session::write_atomic(path, self.to_toml().as_bytes())
    }
}

/// A TOML string.
fn quote(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The values a settings line can hold.
#[derive(Debug, PartialEq, Eq)]
enum Value {
    Str(String),
    Bool(bool),
    Int(i64),
    List(Vec<String>),
}

impl Value {
    /// The value at the start of `text` (after `=`); a `#` comment may follow it.
    fn parse(text: &str) -> Option<Self> {
        let text = text.trim_start();
        if let Some(rest) = text.strip_prefix('[') {
            let mut items = Vec::new();
            let mut rest = rest.trim_start();
            while let Some(after) = rest.strip_prefix('"') {
                let (item, tail) = string(after)?;
                items.push(item);
                rest = tail.trim_start();
                rest = rest.strip_prefix(',').unwrap_or(rest).trim_start();
            }
            return rest.starts_with(']').then_some(Self::List(items));
        }
        if let Some(rest) = text.strip_prefix('"') {
            return string(rest).map(|(s, _)| Self::Str(s));
        }
        let word = text.split('#').next().unwrap_or("").trim();
        match word {
            "true" => Some(Self::Bool(true)),
            "false" => Some(Self::Bool(false)),
            _ => word.parse().ok().map(Self::Int),
        }
    }
}

/// The string up to its closing quote, and what follows it.
fn string(text: &str) -> Option<(String, &str)> {
    let mut out = String::new();
    let mut chars = text.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some((out, &text[i + 1..])),
            '\\' => out.push(chars.next()?.1),
            c => out.push(c),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_through_the_file() {
        let mut s = Settings::default();
        assert_eq!(Settings::parse(&s.to_toml()), s);
        s.mouse = false;
        s.theme = Theme::Light;
        s.discreet = true;
        s.glyphs = Glyphs::Ascii;
        s.confirmations = false;
        s.boss_view = BossView::Top;
        s.boss_keys = vec![KeyCode::Char('"'), KeyCode::Char('#'), KeyCode::F(9)];
        s.last_character = Character::Regent;
        s.last_ascension = 7;
        assert_eq!(Settings::parse(&s.to_toml()), s);
    }

    #[test]
    fn a_bad_file_falls_back_to_the_defaults() {
        let s = Settings::parse(
            "art = 3\nmouse = \"yes\"\nlast_ascension = 99 # too high\nnonsense\ntheme = \"muted\" # ok\nboss_keys = [\"F99\"]\n",
        );
        assert!(s.mouse);
        assert_eq!(s.last_ascension, MAX_ASCENSION);
        assert_eq!(s.theme, Theme::Muted);
        assert_eq!(s.boss_keys, Settings::default().boss_keys);
    }

    #[test]
    fn the_terminal_names_its_background() {
        assert_eq!(Theme::from_colorfgbg("15;0"), Some(Theme::Dark));
        assert_eq!(Theme::from_colorfgbg("0;15"), Some(Theme::Light));
        assert_eq!(Theme::from_colorfgbg("0;default;7"), Some(Theme::Light));
        assert_eq!(Theme::from_colorfgbg("7;8"), Some(Theme::Dark));
        assert_eq!(Theme::from_colorfgbg(""), None);
        assert_eq!(Theme::from_colorfgbg("light"), None);
        assert_eq!(Theme::from_name("muted"), Some(Theme::Muted));
        assert_eq!(Theme::from_name("blue"), None);
    }

    #[test]
    fn every_row_cycles_back_to_where_it_started() {
        let start = Settings::default();
        for row in Row::ALL {
            let mut s = start.clone();
            let mut seen = vec![s.value(row)];
            for _ in 0..10 {
                s.cycle(row, true);
                if s == start {
                    break;
                }
                seen.push(s.value(row));
            }
            assert_eq!(s, start, "{row:?}");
            assert!(seen.len() >= 2, "{row:?} has one value");
            s.cycle(row, false);
            s.cycle(row, true);
            assert_eq!(s, start, "{row:?} backwards");
        }
    }
}
