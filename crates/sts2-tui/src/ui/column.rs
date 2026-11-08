//! The one column every screen is drawn in, like a phone held upright.
//!
//! Three fifths of the terminal's width between [`MIN_WIDTH`] and [`MAX_WIDTH`] cells and
//! centred; on a big terminal ([`super::scale::is_big`]) seven eighths of it, at most
//! [`BIG_MAX_WIDTH`], so the screen is filled rather than framed. Its top is where a screen
//! [`DESIGN_HEIGHT`] rows tall would start when centred, but at most [`MAX_TOP`] rows down. A
//! wider terminal adds margin around it; a taller one hands the rows to the screen, which
//! stacks its parts from the top of the column down ([`Stack`]).

use ratatui::layout::Rect;

/// The column's narrowest on a terminal that wide, margins included: the width the screens
/// are designed for.
pub const MIN_WIDTH: u16 = 48;
/// The column's widest, margins included, short of a big terminal: wider than this a row is
/// hard to read across.
pub const MAX_WIDTH: u16 = 92;
/// The column's widest on a big terminal, margins included.
pub const BIG_MAX_WIDTH: u16 = 152;
/// The height the screens are designed for: on a taller terminal the column starts this far
/// above the middle.
pub const DESIGN_HEIGHT: u16 = 30;
/// The most rows left over the column: the rest of a tall terminal's rows go to the screen.
pub const MAX_TOP: u16 = 3;

/// The column of `area`: centred, one cell of margin each side when it is that wide, from its
/// top row down to the bottom of `area`.
#[must_use]
pub fn of(area: Rect) -> Rect {
    let width = if super::scale::is_big(area) {
        big_width(area.width)
    } else {
        area.width
            .min((area.width * 3 / 5).clamp(MIN_WIDTH, MAX_WIDTH))
    };
    let margin = u16::from(width >= 30);
    let top = (area.height.saturating_sub(DESIGN_HEIGHT) / 2).min(MAX_TOP);
    Rect {
        x: area.x + (area.width - width) / 2 + margin,
        y: area.y + top,
        width: width - 2 * margin,
        height: area.height - top,
    }
}

/// The column's width, margins included, on a big terminal `width` wide: seven eighths of it,
/// at most [`BIG_MAX_WIDTH`].
#[must_use]
pub const fn big_width(width: u16) -> u16 {
    let seven_eighths = width - width / 8;
    if seven_eighths > BIG_MAX_WIDTH {
        BIG_MAX_WIDTH
    } else {
        seven_eighths
    }
}

/// `area` cut to its middle `width` columns.
#[must_use]
pub fn middle(area: Rect, width: u16) -> Rect {
    let width = width.min(area.width);
    Rect {
        x: area.x + (area.width - width) / 2,
        width,
        ..area
    }
}

/// Rows of an area handed out from the top while they last.
#[derive(Clone, Copy, Debug)]
pub struct Stack {
    area: Rect,
    used: u16,
}

impl Stack {
    #[must_use]
    pub const fn new(area: Rect) -> Self {
        Self { area, used: 0 }
    }

    /// The next `rows` rows (fewer when the area runs out).
    pub fn take(&mut self, rows: u16) -> Rect {
        let rows = rows.min(self.left());
        let rect = Rect {
            y: self.area.y + self.used,
            height: rows,
            ..self.area
        };
        self.used += rows;
        rect
    }

    /// Leaves `rows` rows empty.
    pub fn skip(&mut self, rows: u16) {
        self.take(rows);
    }

    /// The rows not handed out yet.
    #[must_use]
    pub const fn left(&self) -> u16 {
        self.area.height - self.used
    }

    /// Everything not handed out yet.
    #[must_use]
    pub const fn rest(&self) -> Rect {
        Rect {
            y: self.area.y + self.used,
            height: self.area.height - self.used,
            ..self.area
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_column_is_centred_and_starts_where_a_design_screen_would() {
        assert_eq!(of(Rect::new(0, 0, 45, 30)), Rect::new(1, 0, 43, 30));
        assert_eq!(of(Rect::new(0, 0, 80, 24)), Rect::new(17, 0, 46, 24));
        assert_eq!(
            of(Rect::new(0, 0, 120, 34)),
            Rect::new(25, 2, 70, 32),
            "three fifths short of a big screen"
        );
        assert_eq!(
            of(Rect::new(0, 0, 120, 35)),
            Rect::new(8, 2, 103, 33),
            "seven eighths on a big one"
        );
        assert_eq!(of(Rect::new(0, 0, 160, 45)), Rect::new(11, 3, 138, 42));
        assert_eq!(
            of(Rect::new(0, 0, 200, 55)),
            Rect::new(25, 3, 150, 52),
            "at most 152 with its margins"
        );
        assert_eq!(of(Rect::new(0, 0, 20, 10)), Rect::new(0, 0, 20, 10));
        let mut stack = Stack::new(Rect::new(0, 0, 10, 5));
        assert_eq!(stack.take(2), Rect::new(0, 0, 10, 2));
        stack.skip(1);
        assert_eq!(stack.take(9), Rect::new(0, 3, 10, 2), "what is left");
        assert_eq!(stack.left(), 0);
    }
}
