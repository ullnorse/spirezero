//! The ASCII glyph set (`Glyphs: ASCII` in the settings).
//!
//! Every glyph the screens draw is mapped through one table after the frame is drawn, for a
//! terminal font without them. Each stand-in is one cell wide, like the glyph it replaces, so
//! nothing moves. Display state only: a thread-local set from the shell's settings at the
//! start of each frame.

use std::cell::Cell;

use ratatui::buffer::Buffer;

use crate::settings::Glyphs;

thread_local! {
    static ASCII: Cell<bool> = const { Cell::new(false) };
}

/// Whether the frames are drawn in ASCII.
#[must_use]
pub fn is_ascii() -> bool {
    ASCII.get()
}

/// Draws the next frames in `glyphs`.
pub fn set(glyphs: Glyphs) {
    ASCII.set(glyphs == Glyphs::Ascii);
}

/// The ASCII stand-in for `glyph`, if it has one: `E` energy, `*` a star, `!` an attack, `#`
/// Block, `+` a buff, `-` a debuff, `=` an HP bar (`[` and `]` its ends).
#[must_use]
pub const fn ascii(glyph: char) -> Option<char> {
    Some(match glyph {
        'ϟ' => 'E',
        '★' => '*',
        '⚔' => '!',
        '▣' | '≡' => '#',
        '▼' | '−' | '─' | '═' | '▔' => '-',
        '█' => '=',
        '▀' => '"',
        '▒' => ':',
        '░' | '·' => '.',
        '◈' => 'o',
        '◆' | '●' => '@',
        '◉' => '$',
        '♥' => 'H',
        '✚' => '&',
        '✦' => '^',
        '✶' => '%',
        '»' => '}',
        '☠' => 'X',
        '✗' | '✕' | '×' => 'x',
        '✓' => 'v',
        '▸' | '›' | '→' | '▶' => '>',
        '◀' => '<',
        '…' => '~',
        '▄' | '▌' | '▁' => '_',
        '▕' => '[',
        '▏' => ']',
        '│' | '║' => '|',
        '▲' | '┌' | '┐' | '└' | '┘' | '╔' | '╗' | '╚' | '╝' | '├' | '┤' | '┬' | '┴' | '┼' => {
            '+'
        }
        _ => return None,
    })
}

/// Maps every glyph of the frame drawn in `buffer` through the table, when ASCII is asked for.
pub fn apply(buffer: &mut Buffer) {
    if !ASCII.get() {
        return;
    }
    for cell in &mut buffer.content {
        let symbol = cell.symbol();
        let mut chars = symbol.chars();
        if let (Some(c), None) = (chars.next(), chars.next())
            && let Some(plain) = ascii(c)
        {
            cell.set_char(plain);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Cell;
    use ratatui::layout::Rect;

    #[test]
    fn every_stand_in_is_one_ascii_cell() {
        for c in "ϟ★⚔▣▲▼█▒░◈◆◉♥✚✦✶»☠✗✕×✓▸›→▶◀≡…·−▌▁▔▕▏─═│║┌┐└┘╔╗╚╝".chars()
        {
            let plain = ascii(c).unwrap_or_else(|| panic!("{c} has no stand-in"));
            assert!(plain.is_ascii_graphic(), "{c} → {plain}");
        }
        assert_eq!(ascii('a'), None);
    }

    #[test]
    fn the_frame_is_mapped_only_when_asked() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 6, 1));
        buffer.set_string(0, 0, "ϟ1 ⚔ 3", ratatui::style::Style::new());
        set(Glyphs::Unicode);
        apply(&mut buffer);
        assert_eq!(buffer.cell((0, 0)).map(Cell::symbol), Some("ϟ"));
        set(Glyphs::Ascii);
        apply(&mut buffer);
        let row: String = (0..6)
            .filter_map(|x| buffer.cell((x, 0)))
            .map(Cell::symbol)
            .collect();
        assert_eq!(row, "E1 ! 3");
        set(Glyphs::Unicode);
    }
}
