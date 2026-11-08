//! Buttons, the way a phone game offers every choice.
//!
//! Full-width rows with the key at the left, clickable over all their rows; the one the
//! keyboard focus rests on is marked `▸` and drawn on the highlight band. The choices of a screen
//! share one frame ([`draw_list`]); a screen's main button (End Turn, Start, Proceed) has a
//! double frame of its own ([`draw_primary`]).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::border;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::hit::{Hits, Target, UiAction};
use crate::ui::flow;
use crate::ui::scale;
use crate::ui::theme::{self, Role, style};

/// A row's text in full, shown while the focus rests on it: what the row says before the text
/// (a card's cost and name) and the text itself, wrapped under its first row in the text's
/// own column.
#[derive(Clone, Debug, Default)]
pub struct Expanded {
    pub prefix: Vec<Span<'static>>,
    pub text: Vec<Span<'static>>,
    /// The text starts on the row under the prefix, this many cells in from the key's column
    /// (a big screen's two-row card: the name on the first row, the text on the second);
    /// `None` puts it beside the prefix.
    pub below: Option<usize>,
}

/// The fewest cells the expanded text wraps in; a narrower column puts it under the key
/// (unless it fits in the column on one row).
const MIN_TEXT_WIDTH: usize = 16;

/// One choice: `1  ϟ1 Strike   Deal 6 damage`, its detail rows under it.
#[derive(Clone, Debug, Default)]
pub struct Button {
    /// The key that presses it (`1`, `e`), at the left.
    pub key: String,
    pub line: Vec<Span<'static>>,
    /// Kept at the right end of the first row (a price).
    pub right: Vec<Span<'static>>,
    /// Rows under the first, indented under its text (what the choice does).
    pub detail: Vec<Line<'static>>,
    /// What a click presses; `None` for a row that is only shown.
    pub target: Option<Target>,
    /// Drawn dim: a choice that cannot be taken, or a card that cannot be played (a click
    /// says why).
    pub dim: bool,
    /// Highlighted like the focused row: the card waiting for its target, a picked card.
    pub selected: bool,
    /// The most detail rows shown, the last cut with `…` (0: all).
    pub max_detail: usize,
    /// The row with its text in full, in place of `line` while the focus rests on it (or it is
    /// selected); `None` for a row that is always one line.
    pub expanded: Option<Expanded>,
    /// The text in full on every row, not only the focused one (a wide column).
    pub open: bool,
    /// Whether the row gets a blank padding line; `None` leaves it to the screen's scale.
    pub pad: Option<bool>,
    /// The row has a detail sheet: marked `›` at its right end.
    pub more: Option<char>,
}

impl Button {
    #[must_use]
    pub fn new(key: impl Into<String>, line: Vec<Span<'static>>, target: Option<Target>) -> Self {
        Self {
            key: key.into(),
            line,
            target,
            ..Self::default()
        }
    }

    /// A button with a plain label.
    #[must_use]
    pub fn label(key: impl Into<String>, label: impl Into<String>, target: Option<Target>) -> Self {
        Self::new(key, vec![Span::raw(label.into())], target)
    }

    #[must_use]
    pub fn right(mut self, right: Vec<Span<'static>>) -> Self {
        self.right = right;
        self
    }

    #[must_use]
    pub fn detail(mut self, detail: Vec<Line<'static>>) -> Self {
        self.detail = detail;
        self
    }

    #[must_use]
    pub const fn dim(mut self, dim: bool) -> Self {
        self.dim = dim;
        self
    }

    /// Marks the row `›`: it has details (the detail key or a right click opens them).
    #[must_use]
    pub const fn more(mut self, more: bool) -> Self {
        self.more = if more { Some('›') } else { None };
        self
    }

    #[must_use]
    pub const fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    #[must_use]
    pub const fn max_detail(mut self, rows: usize) -> Self {
        self.max_detail = rows;
        self
    }

    /// The row's text in full, for while the focus rests on it.
    #[must_use]
    pub fn expanded(mut self, expanded: Expanded) -> Self {
        self.expanded = Some(expanded);
        self
    }

    /// The text in full whether or not the focus rests on the row.
    #[must_use]
    pub const fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Pads the row with a blank line (or not), whatever the screen's scale says.
    #[must_use]
    pub const fn pad(mut self, pad: bool) -> Self {
        self.pad = Some(pad);
        self
    }

    /// The detail wrapped to `width`, cut to [`Button::max_detail`] rows.
    fn detail_lines(&self, width: usize) -> Vec<Line<'static>> {
        let mut lines = flow::wrap_all(&self.detail, width);
        if self.max_detail > 0 && lines.len() > self.max_detail {
            lines.truncate(self.max_detail);
            if let Some(last) = lines.last_mut() {
                *last = Line::from(flow::ellipsized(&last.spans, width));
            }
        }
        lines
    }

    /// The rows of the text in full `width` wide after `indent` columns: the first begins
    /// with the prefix, the rest are indented to the text's column (under the key when the
    /// column would be too narrow).
    fn expanded_rows(expanded: &Expanded, width: usize, indent: usize) -> Vec<Vec<Span<'static>>> {
        let room = width.saturating_sub(indent);
        if let Some(below) = expanded.below {
            let mut rows = vec![flow::fit(&expanded.prefix, room)];
            let below = below.min(room.saturating_sub(MIN_TEXT_WIDTH));
            let line = Line::from(expanded.text.clone());
            for line in flow::wrap(&line, room.saturating_sub(below).max(1)) {
                let mut spans = vec![Span::raw(" ".repeat(indent + below))];
                spans.extend(line.spans);
                rows.push(spans);
            }
            return rows;
        }
        let prefix_width: usize = expanded
            .prefix
            .iter()
            .map(|s| flow::width(&s.content))
            .sum();
        // The text in its column while the column is wide enough, or the text fits in it.
        let beside = room.saturating_sub(prefix_width);
        let line = Line::from(expanded.text.clone());
        let mut text = flow::wrap(&line, beside.max(1));
        let column = if beside >= MIN_TEXT_WIDTH || (beside > 0 && text.len() <= 1) {
            prefix_width
        } else {
            text = flow::wrap(&line, room.max(1));
            0
        };
        let mut rows = Vec::new();
        let mut first = expanded.prefix.clone();
        let mut text = text.into_iter();
        if column > 0 {
            first.extend(text.next().unwrap_or_default().spans);
        }
        rows.push(flow::fit(&first, room));
        for line in text {
            let mut spans = vec![Span::raw(" ".repeat(indent + column))];
            spans.extend(line.spans);
            rows.push(spans);
        }
        rows
    }

    /// The button's rows `width` wide, keys padded to `key_width`, the focused one marked
    /// (and its text in full when it has one).
    fn lines(&self, width: usize, key_width: usize, hits: &Hits) -> Vec<Line<'static>> {
        let focused = self.target.is_some_and(|t| hits.focused(t));
        self.rows(width, key_width, focused)
    }

    /// [`Button::lines`] with the focus given.
    fn rows(&self, width: usize, key_width: usize, focused: bool) -> Vec<Line<'static>> {
        let indent = indent(key_width);
        let muted = style(Role::Muted);
        // Quieter than what they pick: the costs and names carry the colour.
        let key_style = if self.dim {
            muted
        } else {
            Style::new().fg(Color::Gray)
        };
        let marker = if focused { "▸" } else { " " };
        let mark = usize::from(self.more.is_some());
        let right_width: usize = self
            .right
            .iter()
            .map(|s| flow::width(&s.content))
            .sum::<usize>()
            + mark;
        let room = width.saturating_sub(indent + right_width + usize::from(right_width > 0));
        let expanded = self
            .expanded
            .as_ref()
            .filter(|_| focused || self.selected || self.open)
            .map(|e| Self::expanded_rows(e, room + indent, indent));
        let mut first = vec![Span::styled(
            format!("{marker}{:<key_width$}  ", self.key),
            key_style,
        )];
        let mut more = Vec::new();
        match expanded {
            Some(rows) => {
                let mut rows = rows.into_iter();
                first.extend(rows.next().unwrap_or_default());
                more.extend(rows);
            }
            None => first.extend(flow::fit(&self.line, room)),
        }
        let used: usize = first.iter().map(|s| flow::width(&s.content)).sum();
        first.push(Span::raw(
            " ".repeat(width.saturating_sub(used + right_width)),
        ));
        first.extend(self.right.iter().cloned());
        if let Some(mark) = self.more {
            first.push(Span::styled(mark.to_string(), style(Role::Muted)));
        }
        let mut lines = vec![Line::from(first)];
        for mut spans in more {
            let used: usize = spans.iter().map(|s| flow::width(&s.content)).sum();
            spans.push(Span::raw(" ".repeat(width.saturating_sub(used))));
            lines.push(Line::from(spans));
        }
        for line in self.detail_lines(width.saturating_sub(indent)) {
            let mut spans = vec![Span::raw(" ".repeat(indent))];
            spans.extend(line.spans);
            let used: usize = spans.iter().map(|s| flow::width(&s.content)).sum();
            spans.push(Span::raw(" ".repeat(width.saturating_sub(used))));
            lines.push(Line::from(spans).style(Style::new().fg(Color::Gray)));
        }
        // A tall screen pads every row with a blank line, so a row is easy to hit and the
        // focused one is a two-row band.
        if self.pad.unwrap_or_else(scale::padded) {
            lines.push(Line::from(Span::raw(" ".repeat(width))));
        }
        lines
            .into_iter()
            .map(|line| {
                let line = if self.dim { dimmed(line) } else { line };
                if self.selected || focused {
                    flow::banded(line)
                } else {
                    line
                }
            })
            .collect()
    }

    /// The rows the button takes `width` wide, with its text in full if `expanded`.
    fn height(&self, width: usize, key_width: usize, expanded: bool) -> usize {
        self.rows(width, key_width, expanded && self.expanded.is_some())
            .len()
    }
}

/// The number keys `keys` print, as a hint says them: `1-4 pick`, `1-0 pick` when a page's
/// tenth row is `0`; empty when no row has a number.
#[must_use]
pub fn pick_hint<'a>(keys: impl IntoIterator<Item = &'a str>) -> String {
    let mut digits: Vec<u8> = keys
        .into_iter()
        .filter_map(|k| k.parse::<u8>().ok())
        .map(|d| if d == 0 { 10 } else { d })
        .collect();
    digits.sort_unstable();
    match (digits.first(), digits.last()) {
        (Some(&first), Some(&last)) if first == last => format!("{first} pick"),
        (Some(&first), Some(&last)) => {
            let last = if last == 10 {
                "0".to_owned()
            } else {
                last.to_string()
            };
            format!("{first}-{last} pick")
        }
        _ => String::new(),
    }
}

/// The columns before a row's text: the focus marker, the key and two spaces.
const fn indent(key_width: usize) -> usize {
    key_width + 3
}

/// `line` in the muted colour, its bold kept.
fn dimmed(line: Line<'static>) -> Line<'static> {
    let muted = style(Role::Muted);
    Line::from(
        line.spans
            .into_iter()
            .map(|s| Span::styled(s.content, s.style.fg(muted.fg.unwrap_or(Color::DarkGray))))
            .collect::<Vec<_>>(),
    )
}

fn key_width(buttons: &[Button]) -> usize {
    buttons
        .iter()
        .map(|b| flow::width(&b.key))
        .max()
        .unwrap_or(0)
}

/// Whether the buttons are told apart by a rule: when one has rows under it.
fn ruled(buttons: &[Button]) -> bool {
    buttons.iter().any(|b| !b.detail.is_empty())
}

/// The rows [`draw_list`] needs for `buttons` `width` wide, its frame included if `framed`,
/// with room for the longest text in full, so the list is as tall whichever row the focus
/// rests on.
#[must_use]
pub fn list_height(buttons: &[Button], width: u16, framed: bool) -> u16 {
    let inner = usize::from(width.saturating_sub(2 * u16::from(framed)));
    let keys = key_width(buttons);
    let extra = buttons
        .iter()
        .map(|b| b.height(inner, keys, true) - b.height(inner, keys, false))
        .max()
        .unwrap_or(0);
    closed_height(buttons, width, framed) + flow::coord(extra)
}

/// [`list_height`] with no text open: the rows the list takes at the least (it scrolls to
/// the focused row when its text opens in less room than [`list_height`]).
#[must_use]
pub fn closed_height(buttons: &[Button], width: u16, framed: bool) -> u16 {
    let inner = usize::from(width.saturating_sub(2 * u16::from(framed)));
    let keys = key_width(buttons);
    let rows: usize = buttons.iter().map(|b| b.height(inner, keys, false)).sum();
    let rules = if ruled(buttons) {
        buttons.len().saturating_sub(1)
    } else {
        0
    };
    flow::coord(rows + rules + 2 * usize::from(framed))
}

/// Draws `buttons` one under another in `area`, in one frame if `framed`.
///
/// From row `scroll` of the list, moved as little as needed to show the focused button whole;
/// each is registered over all its rows. Returns how far the list can scroll.
pub fn draw_list(
    frame: &mut Frame<'_>,
    area: Rect,
    buttons: &[Button],
    framed: bool,
    scroll: usize,
    hits: &mut Hits,
) -> usize {
    draw_list_titled(frame, area, buttons, framed, None, scroll, hits)
}

/// [`draw_list`] with `title` on the frame's top edge (a card choice's prompt).
///
/// The title is cut to the edge; an unframed list has nowhere for it. An unframed list that
/// scrolls marks its first and last rows' right ends with `▲` and `▼` instead of the frame.
pub fn draw_list_titled(
    frame: &mut Frame<'_>,
    area: Rect,
    buttons: &[Button],
    framed: bool,
    title: Option<&str>,
    scroll: usize,
    hits: &mut Hits,
) -> usize {
    if area.is_empty() {
        return 0;
    }
    let inner = if framed {
        Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        }
    } else {
        area
    };
    let width = usize::from(inner.width);
    let keys = key_width(buttons);
    let rule = Line::from(Span::styled("─".repeat(width), style(Role::Muted)));
    let mut rows: Vec<(Line<'static>, Option<Target>)> = Vec::new();
    let mut focused: Option<std::ops::Range<usize>> = None;
    for (i, button) in buttons.iter().enumerate() {
        if i > 0 && ruled(buttons) {
            rows.push((rule.clone(), None));
        }
        let from = rows.len();
        rows.extend(
            button
                .lines(width, keys, hits)
                .into_iter()
                .map(|l| (l, button.target)),
        );
        if button.target.is_some_and(|t| hits.focused(t)) {
            focused = Some(from..rows.len());
        }
    }
    let height = usize::from(inner.height);
    let max_scroll = rows.len().saturating_sub(height);
    let mut scroll = scroll.min(max_scroll);
    if let Some(range) = focused {
        scroll = scroll
            .min(range.start)
            .max(range.end.saturating_sub(height));
    }
    let mut lines = Vec::new();
    for (n, (line, target)) in rows.into_iter().skip(scroll).take(height).enumerate() {
        if let Some(target) = target {
            hits.push(
                Rect {
                    y: inner.y + flow::coord(n),
                    height: 1,
                    ..inner
                },
                target,
            );
        }
        lines.push(line);
    }
    if framed {
        let mut block = Block::bordered().border_style(style(Role::Muted));
        if let Some(title) = title {
            let room = usize::from(area.width.saturating_sub(2 + 2 * u16::from(scroll > 0)));
            let title = format!(" {} ", flow::truncated(title, room.saturating_sub(2)));
            block = block.title(Line::from(Span::styled(title, style(Role::Key))));
        }
        if scroll > 0 {
            block = block.title(Line::from(" ▲ ").right_aligned());
        }
        if scroll < max_scroll {
            block = block.title_bottom(Line::from(" ▼ more ").right_aligned());
        }
        frame.render_widget(block, area);
    }
    frame.render_widget(Paragraph::new(lines), inner);
    if !framed && inner.width > 0 {
        let mark = |frame: &mut Frame<'_>, y: u16, glyph: &str| {
            let cell = Rect {
                x: inner.right() - 1,
                y,
                width: 1,
                height: 1,
            };
            frame.render_widget(Paragraph::new(Span::styled(glyph, style(Role::Key))), cell);
        };
        if scroll > 0 {
            mark(frame, inner.y, "▲");
        }
        if scroll < max_scroll {
            mark(frame, inner.bottom() - 1, "▼");
        }
    }
    max_scroll
}

/// Draws the screen's main button in `area`.
///
/// A double frame around `e   End Turn` from three rows, else one bold row; registered unless
/// it cannot be pressed now; as wide as its label wants, centred; filled with the highlight band
/// to its edges while the focus rests on it.
pub fn draw_primary(
    frame: &mut Frame<'_>,
    area: Rect,
    key: &str,
    label: &str,
    action: UiAction,
    enabled: bool,
    hits: &mut Hits,
) {
    if area.is_empty() {
        return;
    }
    let target = Target::Button(action);
    let focused = enabled && hits.focused(target);
    // An outline until the focus rests on it; then filled, the band to its edges.
    let (edge, text) = match (enabled, focused) {
        (false, _) => (style(Role::Muted), style(Role::Muted)),
        (true, false) => (style(Role::Body), Style::new().add_modifier(Modifier::BOLD)),
        (true, true) => (
            Style::new().fg(theme::band()),
            flow::highlighted(Style::new()),
        ),
    };
    let key_style = if enabled { style(Role::Key) } else { text };
    let marker = if focused { "▸" } else { " " };
    let title = Line::from(vec![
        Span::styled(format!("{marker}{key}"), key_style.patch(text)),
        Span::styled(format!("   {label} "), text),
    ]);
    // A button, not a bar: as wide as its label wants, centred in the row it is given.
    let wide = flow::coord(flow::width(key) + flow::width(label) + PRIMARY_PADDING)
        .max(PRIMARY_MIN_WIDTH)
        .min(area.width);
    let area = Rect {
        x: area.x + (area.width - wide) / 2,
        width: wide,
        ..area
    };
    if enabled {
        hits.push(area, target);
    }
    let row = if area.height >= 3 {
        // Focused, the frame is half blocks in the band's colour: the band fills the box to
        // the middle of its edge cells, so it covers the box whole and never shows outside.
        let block = Block::bordered()
            .border_set(if focused {
                border::PROPORTIONAL_WIDE
            } else {
                border::DOUBLE
            })
            .border_style(edge);
        let inside = block.inner(area);
        frame.render_widget(block, area);
        if focused {
            frame.render_widget(Block::new().style(text), inside);
        }
        Rect {
            x: area.x + 1,
            y: area.y + area.height / 2,
            width: area.width.saturating_sub(2),
            height: 1,
        }
    } else {
        Rect { height: 1, ..area }
    };
    let row_style = if focused { text } else { Style::new() };
    frame.render_widget(Paragraph::new(title).centered().style(row_style), row);
}

/// The fewest columns the main button takes, and the room it leaves around its key and label.
const PRIMARY_MIN_WIDTH: u16 = 28;
const PRIMARY_PADDING: usize = 20;

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn render(
        draw: impl FnOnce(&mut Frame<'_>, &mut Hits),
        focus: Option<Target>,
    ) -> (Vec<String>, Hits) {
        render_at(20, draw, focus)
    }

    fn render_at(
        width: u16,
        draw: impl FnOnce(&mut Frame<'_>, &mut Hits),
        focus: Option<Target>,
    ) -> (Vec<String>, Hits) {
        let mut terminal = Terminal::new(TestBackend::new(width, 6)).expect("backend");
        let mut hits = Hits::new();
        hits.set_focus(focus);
        terminal.draw(|f| draw(f, &mut hits)).expect("draw");
        let rows = terminal
            .backend()
            .buffer()
            .content()
            .chunks(usize::from(width))
            .map(|r| {
                r.iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect();
        (rows, hits)
    }

    #[test]
    fn the_main_button_is_framed_and_clickable() {
        let (rows, hits) = render(
            |f, hits| {
                draw_primary(
                    f,
                    Rect::new(0, 0, 18, 3),
                    "e",
                    "End Turn",
                    UiAction::EndTurn,
                    true,
                    hits,
                );
            },
            None,
        );
        assert_eq!(
            rows[..3],
            [
                "╔════════════════╗",
                "║  e   End Turn  ║",
                "╚════════════════╝"
            ]
        );
        assert_eq!(
            hits.rect_of(Target::Button(UiAction::EndTurn)),
            Some(Rect::new(0, 0, 18, 3))
        );
    }

    #[test]
    fn a_list_is_one_frame_of_full_width_rows() {
        let pick = |n| Some(Target::Button(UiAction::Pick(n)));
        let buttons = [
            Button::label("1", "Dive In", pick(1)).detail(vec![Line::from("Lose 8 HP.")]),
            Button::label("2", "Leave", None)
                .right(vec![Span::raw("9g")])
                .dim(true),
        ];
        assert_eq!(list_height(&buttons, 20, true), 6);
        let (rows, hits) = render(
            |f, hits| {
                draw_list(f, Rect::new(0, 0, 20, 6), &buttons, true, 0, hits);
            },
            None,
        );
        assert_eq!(
            rows,
            [
                "┌──────────────────┐",
                "│ 1  Dive In       │",
                "│    Lose 8 HP.    │",
                "│──────────────────│",
                "│ 2  Leave       9g│",
                "└──────────────────┘",
            ]
        );
        assert_eq!(
            hits.regions().iter().map(|r| r.rect).collect::<Vec<_>>(),
            [Rect::new(1, 1, 18, 1), Rect::new(1, 2, 18, 1)],
            "both rows of the first; the second cannot be taken"
        );
        let (rows, _) = render(
            |f, hits| {
                draw_list(f, Rect::new(0, 0, 20, 6), &buttons, true, 0, hits);
            },
            pick(1),
        );
        assert_eq!(rows[1], "│▸1  Dive In       │", "the focused row is marked");
    }

    #[test]
    fn the_focused_row_opens_its_text_in_its_own_column() {
        let card = |n: usize| {
            Button::new(
                n.to_string(),
                vec![Span::raw("1 Bash  Deal 8 damage")],
                Some(Target::Button(UiAction::Pick(n))),
            )
            .expanded(Expanded {
                prefix: vec![Span::raw("1 Bash  ")],
                text: vec![Span::raw("Deal 8 damage. Apply 2 Vulnerable to it.")],
                below: None,
            })
        };
        let buttons = [card(1), card(2)];
        assert_eq!(closed_height(&buttons, 30, false), 2);
        assert_eq!(
            list_height(&buttons, 30, false),
            4,
            "room for one text in full"
        );
        let (rows, hits) = render_at(
            30,
            |f, hits| {
                draw_list(f, Rect::new(0, 0, 30, 4), &buttons, false, 0, hits);
            },
            Some(Target::Button(UiAction::Pick(2))),
        );
        assert_eq!(
            rows[..4],
            [
                " 1  1 Bash  Deal 8 damage",
                "▸2  1 Bash  Deal 8 damage.",
                "            Apply 2 Vulnerable",
                "            to it.",
            ]
        );
        assert_eq!(
            hits.rect_of(Target::Button(UiAction::Pick(2))),
            Some(Rect::new(0, 3, 30, 1)),
            "the last of its rows (each is registered)"
        );
        // Short of rows the list scrolls to the focused row, and says so at the right end
        // of its first row (no frame to carry the mark).
        let (rows, _) = render_at(
            30,
            |f, hits| {
                draw_list(f, Rect::new(0, 0, 30, 3), &buttons, false, 0, hits);
            },
            Some(Target::Button(UiAction::Pick(2))),
        );
        assert_eq!(rows[0], "▸2  1 Bash  Deal 8 damage.   ▲");
    }
}
