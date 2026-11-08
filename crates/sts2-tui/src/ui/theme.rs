//! The look: what each colour means ([`Role`]) and the palette the settings ask for.
//!
//! Screens style by role, never by raw colour, so a colour means one thing everywhere and is
//! always paired with a word or a glyph. The roles use the Okabe–Ito colours (told apart under
//! deuteranopia and protanopia), or their 16-colour fallbacks on a terminal without true colour.
//! Every frame is then recoloured into the palette (light, greys only) after it is drawn.
//! Display state only: thread-locals set from the shell's settings at the
//! start of each frame.

use std::cell::Cell;

use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier, Style};

use crate::settings::Theme;

pub use sts2_view::text::Role;

/// Okabe–Ito vermillion, blue, yellow, reddish purple and sky blue.
const VERMILLION: Color = Color::Rgb(0xD5, 0x5E, 0x00);
const BLUE: Color = Color::Rgb(0x00, 0x72, 0xB2);
const YELLOW: Color = Color::Rgb(0xF0, 0xE4, 0x42);
const PURPLE: Color = Color::Rgb(0xCC, 0x79, 0xA7);
const SKY: Color = Color::Rgb(0x56, 0xB4, 0xE9);
/// Okabe–Ito bluish green: what an upgrade changes.
const GREEN: Color = Color::Rgb(0x00, 0x9E, 0x73);
/// The band behind the focused (or selected) thing: a soft slate that keeps every colour on it
/// readable, where reverse video turned each coloured span into a block of its own.
const BAND: Color = Color::Rgb(0x34, 0x3B, 0x48);
/// [`BAND`] on a light terminal.
const LIGHT_BAND: Color = Color::Rgb(0xDC, 0xE3, 0xEE);
/// The map's elites (and boss): a plain danger red, apart from the attacks' vermillion.
pub const ELITE: Color = Color::Rgb(0xE0, 0x3A, 0x3A);
/// The map's rest sites: firelight amber (Okabe–Ito orange), not the healing blue.
pub const FIRE: Color = Color::Rgb(0xE6, 0x9F, 0x00);

/// The HP the selected card would take, inside an HP bar: a pale tint of the bar's vermillion,
/// so the part about to drain still reads as part of the bar.
pub const LOSS: Color = Color::Rgb(0xFA, 0xC8, 0xA0);
/// [`LOSS`] on a light terminal: a dark shade instead, which stands out there.
const LIGHT_LOSS: Color = Color::Rgb(0x8A, 0x3C, 0x00);
/// The HP the enemies' turn would take, inside your HP bar: a pale rose, apart from a card's
/// preview so the two never read as one.
pub const INCOMING: Color = Color::Rgb(0xF2, 0xA8, 0xB4);
/// [`INCOMING`] on a light terminal.
const LIGHT_INCOMING: Color = Color::Rgb(0x9A, 0x2A, 0x3A);

/// The highlight band's colour, for drawing a shape in it (a focused button's edges).
#[must_use]
pub const fn band() -> Color {
    BAND
}

/// The role's colour on a dark true-colour terminal.
#[must_use]
pub const fn color(role: Role) -> Color {
    match role {
        Role::Attack | Role::Hp | Role::Lethal => VERMILLION,
        Role::Defend => BLUE,
        Role::Buff | Role::Energy | Role::Gold => YELLOW,
        Role::Debuff => PURPLE,
        Role::Heal => SKY,
        Role::Upgrade => GREEN,
        Role::Unknown | Role::Muted => Color::DarkGray,
        Role::Body => Color::Gray,
        Role::Plain | Role::Selected | Role::Title | Role::Key | Role::Emphasis => Color::Reset,
    }
}

/// A card's rarity colour, for its name and its rarity word, as the game frames cards:
/// Uncommon blue, Rare gold, Ancient purple; `None` for the rest (Common and Starter plain).
#[must_use]
pub fn rarity(rarity: &str) -> Option<Style> {
    let color = match rarity {
        "Uncommon" => SKY,
        "Rare" => YELLOW,
        "Ancient" => PURPLE,
        _ => return None,
    };
    Some(Style::new().fg(color))
}

/// Whether a card's rarity is worth its word beside the type (`Attack · Rare`): the rarities a
/// reward or a shop offers.
#[must_use]
pub fn rarity_shown(rarity: &str) -> bool {
    matches!(rarity, "Common" | "Uncommon" | "Rare" | "Ancient")
}

/// `role`'s style.
#[must_use]
pub const fn style(role: Role) -> Style {
    match role {
        Role::Lethal => Style::new()
            .fg(Color::White)
            .bg(VERMILLION)
            .add_modifier(Modifier::BOLD),
        Role::Selected => Style::new().bg(BAND).add_modifier(Modifier::BOLD),
        Role::Title | Role::Key | Role::Energy => {
            Style::new().fg(color(role)).add_modifier(Modifier::BOLD)
        }
        Role::Plain => Style::new(),
        Role::Emphasis => Style::new().add_modifier(Modifier::BOLD),
        _ => Style::new().fg(color(role)),
    }
}

/// The 16-colour stand-in for a role colour.
const fn fallback(color: Color) -> Color {
    match color {
        VERMILLION => Color::LightRed,
        BLUE => Color::Blue,
        YELLOW => Color::Yellow,
        PURPLE => Color::Magenta,
        SKY => Color::Cyan,
        GREEN => Color::Green,
        BAND => Color::DarkGray,
        ELITE => Color::Red,
        FIRE => Color::LightYellow,
        LOSS => Color::White,
        INCOMING => Color::LightMagenta,
        other => other,
    }
}

thread_local! {
    static THEME: Cell<Theme> = const { Cell::new(Theme::Dark) };
    static TRUE_COLOR: Cell<bool> = const { Cell::new(true) };
}

/// Draws the next frames in `palette` ([`Settings::palette`], or the terminal's default).
pub fn set_palette(palette: Theme) {
    THEME.set(palette);
}

/// Whether the terminal draws 24-bit colour (`COLORTERM`); without it the roles use the
/// 16-colour fallbacks.
pub fn set_true_color(on: bool) {
    TRUE_COLOR.set(on);
}

/// Recolours the frame drawn in `buffer` into the palette.
pub fn apply(buffer: &mut Buffer) {
    let theme = THEME.get();
    let true_color = TRUE_COLOR.get();
    if theme == Theme::Dark && true_color {
        return;
    }
    for cell in &mut buffer.content {
        let (fg, bg) = if true_color {
            (cell.fg, cell.bg)
        } else {
            (fallback(cell.fg), fallback(cell.bg))
        };
        cell.fg = recolour(theme, fg, false);
        cell.bg = recolour(theme, bg, true);
    }
}

/// A colour's brightness, 0 to 255.
fn brightness(color: Color) -> u16 {
    match color {
        Color::Rgb(r, g, b) => (u16::from(r) * 3 + u16::from(g) * 6 + u16::from(b)) / 10,
        Color::Black => 0,
        Color::DarkGray | Color::Red | Color::Blue | Color::Magenta => 90,
        Color::Green | Color::Cyan | Color::LightRed | Color::LightBlue | Color::LightMagenta => {
            150
        }
        Color::Gray | Color::Yellow | Color::LightGreen | Color::LightCyan => 190,
        _ => 255,
    }
}

fn recolour(theme: Theme, color: Color, background: bool) -> Color {
    match (theme, color) {
        (_, Color::Reset) | (Theme::Dark, _) => color,
        // Greys only: text in three shades, backgrounds kept only as a dark grey band.
        (Theme::Muted, _) if background => Color::DarkGray,
        (Theme::Muted, c) => match brightness(c) {
            0..=110 => Color::DarkGray,
            111..=200 => Color::Gray,
            _ => Color::White,
        },
        // Light terminals: the dark theme's light text turns dark, and pale colours deepen.
        (Theme::Light, BAND) => LIGHT_BAND,
        (Theme::Light, LOSS) => LIGHT_LOSS,
        (Theme::Light, INCOMING) => LIGHT_INCOMING,
        (Theme::Light, Color::White) => Color::Black,
        (Theme::Light, Color::Black) => Color::White,
        (Theme::Light, Color::Gray) => Color::DarkGray,
        (Theme::Light, Color::DarkGray) => Color::Gray,
        (Theme::Light, Color::Yellow | Color::LightYellow | YELLOW) => Color::Rgb(150, 100, 0),
        (Theme::Light, Color::Cyan | Color::LightCyan | SKY) => Color::Rgb(0, 110, 130),
        (Theme::Light, Color::Green | Color::LightGreen) => Color::Rgb(0, 120, 0),
        (Theme::Light, c) => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    #[test]
    fn the_muted_palette_leaves_no_colour() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 6, 1));
        buffer.set_string(0, 0, "ab", Style::new().fg(Color::Red).bg(Color::Yellow));
        buffer.set_string(2, 0, "cd", Style::new().fg(Color::Rgb(250, 250, 250)));
        buffer.set_string(4, 0, "ef", style(Role::Lethal));
        set_palette(Theme::Muted);
        apply(&mut buffer);
        for cell in &buffer.content {
            for c in [cell.fg, cell.bg] {
                assert!(
                    matches!(
                        c,
                        Color::Reset | Color::White | Color::Gray | Color::DarkGray
                    ),
                    "{c:?}"
                );
            }
        }
        set_palette(Theme::Dark);
    }

    #[test]
    fn attack_and_defend_are_not_red_and_green() {
        assert_ne!(color(Role::Attack), color(Role::Defend));
        for role in [Role::Attack, Role::Defend] {
            assert!(!matches!(color(role), Color::Green | Color::LightGreen));
        }
        assert_eq!(fallback(color(Role::Attack)), Color::LightRed);
        assert_eq!(fallback(color(Role::Defend)), Color::Blue);
    }
}
