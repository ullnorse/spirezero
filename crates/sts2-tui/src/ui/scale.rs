//! How big the screens draw ([`Scale`]).
//!
//! The terminal's size decides, once per frame, whether the column is wide enough for card
//! rows to show their type and whole text, tall enough for rows to get a padding line and
//! the main button a taller frame, and big enough ([`BIG_WIDTH`] by [`BIG_HEIGHT`]) for the
//! column to take most of the screen and the fight to draw its key numbers large: HP bars two
//! rows thick, card rows two rows each. The terminal's font is out of reach, so the space is what gets used: a
//! 45×30 screen draws as designed, a bigger one draws bigger, never emptier. Display state
//! only: a thread-local set from the frame's area at the start of each frame.

use std::cell::Cell;

use ratatui::layout::Rect;

use crate::ui::column;

/// What the frame's size allows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Scale {
    /// The column is [`WIDE_COLUMN`] cells or wider: card rows show the type word and the whole
    /// text, the HP bars grow.
    pub wide: bool,
    /// The frame is [`TALL_FRAME`] rows or taller: every row button gets a padding line and
    /// the main button a five-row frame.
    pub tall: bool,
    /// The frame is [`BIG_WIDTH`] columns and [`BIG_HEIGHT`] rows or more: the column takes
    /// most of the width ([`column::big_width`]) and the screens draw their key numbers large.
    pub big: bool,
}

/// The column width from which card rows show everything.
pub const WIDE_COLUMN: u16 = 70;
/// The frame height from which the rows get their padding.
pub const TALL_FRAME: u16 = 40;
/// The frame size from which the screens draw big.
pub const BIG_WIDTH: u16 = 100;
pub const BIG_HEIGHT: u16 = 35;

thread_local! {
    static SCALE: Cell<Scale> = const {
        Cell::new(Scale {
            wide: false,
            tall: false,
            big: false,
        })
    };
}

/// Whether a frame `area` big draws big.
#[must_use]
pub const fn is_big(area: Rect) -> bool {
    area.width >= BIG_WIDTH && area.height >= BIG_HEIGHT
}

/// The scale of a frame `area` big.
#[must_use]
pub fn of(area: Rect) -> Scale {
    Scale {
        wide: column::of(area).width >= WIDE_COLUMN,
        tall: area.height >= TALL_FRAME,
        big: is_big(area),
    }
}

/// Draws the next frames at the scale `area` allows.
pub fn set(area: Rect) {
    SCALE.set(of(area));
}

/// The scale the frame is drawn at.
#[must_use]
pub fn get() -> Scale {
    SCALE.get()
}

/// Whether row buttons get a padding line.
#[must_use]
pub fn padded() -> bool {
    SCALE.get().tall
}

/// Whether the column is wide enough for the card rows to show everything.
#[must_use]
pub fn wide() -> bool {
    SCALE.get().wide
}

/// Whether the screens draw big: the bars thick, the card rows two rows each where they fit.
#[must_use]
pub fn big() -> bool {
    SCALE.get().big
}

/// The rows a framed main button takes: three at every size (a taller one only took rows from
/// the screen).
#[must_use]
pub const fn primary_rows() -> u16 {
    3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_design_size_is_not_scaled_and_big_terminals_are() {
        assert_eq!(of(Rect::new(0, 0, 45, 30)), Scale::default());
        assert_eq!(of(Rect::new(0, 0, 80, 24)), Scale::default());
        assert_eq!(
            of(Rect::new(0, 0, 99, 35)),
            Scale {
                wide: false,
                tall: false,
                big: false
            },
            "a column 59 wide: not yet big"
        );
        assert_eq!(
            of(Rect::new(0, 0, 120, 35)),
            Scale {
                wide: true,
                tall: false,
                big: true
            }
        );
        assert_eq!(
            of(Rect::new(0, 0, 120, 34)),
            Scale {
                wide: true,
                tall: false,
                big: false
            }
        );
        assert_eq!(
            of(Rect::new(0, 0, 160, 45)),
            Scale {
                wide: true,
                tall: true,
                big: true
            }
        );
    }
}
