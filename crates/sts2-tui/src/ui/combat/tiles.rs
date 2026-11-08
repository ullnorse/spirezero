//! The hand as card tiles on a wide screen: a row of framed cards, as the game lays them out,
//! each with its key, cost and name, its type, and its whole text.
//!
//! ```text
//! ╭────────────────────────╮
//! │▸1  ϟ1 Strike           │
//! │Attack                  │
//! │Deal 6 damage.          │
//! │                        │
//! │                        │
//! ╰────────────────────────╯
//! ```
//!
//! The focused card (or the one waiting for its target) has a bright frame on the highlight
//! band; a card that cannot be played is dim. Tiles wrap to more rows for a big hand, and a
//! hand taller than its area shows the rows from the focused card's.

use std::cell::Cell;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::symbols::border;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::hit::{Hits, Target};
use crate::ui::flow;
use crate::ui::theme::{self, Role, style};

/// One card of the hand as a tile.
#[derive(Clone, Debug)]
pub struct Tile {
    pub key: String,
    pub cost: String,
    pub name: String,
    pub kind: String,
    /// `Common`, `Rare`, ...: the name in its colour, the word beside the type.
    pub rarity: String,
    pub text: Vec<Span<'static>>,
    pub target: Target,
    /// It cannot be played now.
    pub dim: bool,
    /// It waits for its target.
    pub selected: bool,
    /// Its gold glow is on (`ShouldGlowGold`): a gold frame.
    pub glow: bool,
}

/// The rows a tile takes: its frame, its name row, its type row and three rows of text.
pub const HEIGHT: u16 = 7;
/// The rows of a tile's text.
const TEXT_ROWS: usize = 3;
/// The fewest columns a tile takes, the most tiles a row holds, and the blank columns between
/// two tiles.
const MIN_WIDTH: u16 = 24;
const MAX_PER_ROW: usize = 5;
const GAP: u16 = 1;

thread_local! {
    /// How many tiles a row of the last frame's hand held (0: the hand was a list), so the
    /// arrow keys can move between rows as the screen shows them.
    static COLUMNS: Cell<usize> = const { Cell::new(0) };
}

/// The tiles a row of the last frame's hand held; 0 when the hand was a list.
#[must_use]
pub fn columns() -> usize {
    COLUMNS.get()
}

/// Notes that this frame's hand is a list, not tiles.
pub fn drew_list() {
    COLUMNS.set(0);
}

/// How many tiles a row `width` wide holds.
fn per_row(width: u16) -> usize {
    usize::from((width + GAP) / (MIN_WIDTH + GAP)).clamp(1, MAX_PER_ROW)
}

/// The rows `n` tiles take `width` wide.
#[must_use]
pub fn height(n: usize, width: u16) -> u16 {
    flow::coord(n.max(1).div_ceil(per_row(width))) * HEIGHT
}

/// Draws `tiles` in `area`, from the row of the focused one when they do not all fit;
/// registers each tile's rectangle for its card.
pub fn draw(frame: &mut Frame<'_>, area: Rect, tiles: &[Tile], hits: &mut Hits) {
    if area.is_empty() || tiles.is_empty() {
        return;
    }
    let per = per_row(area.width);
    COLUMNS.set(per);
    let width = (area.width + GAP) / flow::coord(per) - GAP;
    let rows_shown = usize::from(area.height / HEIGHT).max(1);
    let focused_row = tiles
        .iter()
        .position(|t| hits.focused(t.target) || t.selected)
        .map_or(0, |i| i / per);
    let first_row = focused_row.saturating_sub(rows_shown - 1);
    for (i, tile) in tiles.iter().enumerate().skip(first_row * per) {
        let (row, col) = (i / per - first_row, i % per);
        if row >= rows_shown {
            break;
        }
        let rect = Rect {
            x: area.x + flow::coord(col) * (width + GAP),
            y: area.y + flow::coord(row) * HEIGHT,
            width,
            height: HEIGHT.min(area.bottom() - (area.y + flow::coord(row) * HEIGHT)),
        };
        draw_tile(frame, rect, tile, hits.focused(tile.target));
        hits.push(rect, tile.target);
    }
}

/// A tile's type row, with the rarity worth saying in its colour: `Attack · Rare`.
fn kind_spans(tile: &Tile) -> Vec<Span<'static>> {
    let mut spans = vec![Span::styled(tile.kind.clone(), style(Role::Muted))];
    if theme::rarity_shown(&tile.rarity) {
        spans.push(Span::styled(" · ", style(Role::Muted)));
        spans.push(Span::styled(
            tile.rarity.clone(),
            theme::rarity(&tile.rarity).unwrap_or_else(|| style(Role::Muted)),
        ));
    }
    spans
}

/// One tile in `rect`.
fn draw_tile(frame: &mut Frame<'_>, rect: Rect, tile: &Tile, focused: bool) {
    let lit = focused || tile.selected;
    let edge = if lit {
        style(Role::Key)
    } else if tile.glow {
        style(Role::Gold)
    } else if tile.dim {
        style(Role::Muted)
    } else {
        style(Role::Body)
    };
    let fill = if lit {
        flow::highlighted(Style::new())
    } else {
        Style::new()
    };
    let block = Block::bordered()
        .border_set(border::ROUNDED)
        .border_style(edge);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    let width = usize::from(inner.width);
    let ink = |s: Style| {
        if tile.dim {
            s.fg(style(Role::Muted).fg.unwrap_or_default())
        } else {
            s
        }
    };
    let marker = if focused { "▸" } else { " " };
    let mut lines = vec![
        Line::from(flow::fit(
            &[
                Span::styled(format!("{marker}{}  ", tile.key), ink(style(Role::Key))),
                Span::styled(format!("ϟ{} ", tile.cost), ink(style(Role::Energy))),
                Span::styled(
                    tile.name.clone(),
                    ink(theme::rarity(&tile.rarity)
                        .unwrap_or_default()
                        .add_modifier(Modifier::BOLD)),
                ),
            ],
            width,
        )),
        Line::from(kind_spans(tile)),
    ];
    let text: Vec<Span<'static>> = tile
        .text
        .iter()
        .map(|s| Span::styled(s.content.clone(), ink(s.style)))
        .collect();
    let mut wrapped = flow::wrap(&Line::from(text), width.max(1));
    if wrapped.len() > TEXT_ROWS {
        wrapped.truncate(TEXT_ROWS);
        if let Some(last) = wrapped.last_mut() {
            *last = Line::from(flow::ellipsized(&last.spans, width));
        }
    }
    lines.extend(wrapped);
    frame.render_widget(Paragraph::new(lines).style(fill), inner);
}
