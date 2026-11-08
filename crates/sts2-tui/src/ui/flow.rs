//! Text laid out by hand, so the hit regions are exactly where the text is drawn: word wrapping
//! that keeps styles, a flow of tokens (relics, potions, powers, buttons) that never breaks one
//! across lines, and the look of what can be clicked.

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthChar;

use crate::hit::{Hits, Target};
use crate::ui::theme::{self, Role};

/// The columns `text` takes.
#[must_use]
pub fn width(text: &str) -> usize {
    text.chars().map(char_width).sum()
}

fn char_width(c: char) -> usize {
    c.width().unwrap_or(0)
}

/// `n` as a terminal coordinate.
#[must_use]
pub fn coord(n: usize) -> u16 {
    u16::try_from(n).unwrap_or(u16::MAX)
}

/// Word-wraps `line` to `width` columns. Styles are kept, a word longer than a row is split, and
/// the rows after the first keep the line's leading indentation (unless it is most of a row).
#[must_use]
pub fn wrap(line: &Line<'_>, width: usize) -> Vec<Line<'static>> {
    let width = width.max(1);
    let cells: Vec<(char, Style)> = line
        .spans
        .iter()
        .flat_map(|s| {
            let style = line.style.patch(s.style);
            s.content.chars().map(move |c| (c, style))
        })
        .filter(|(c, _)| *c != '\n')
        .collect();
    let indent: Vec<(char, Style)> = cells
        .iter()
        .take_while(|(c, _)| *c == ' ')
        .copied()
        .collect();
    let indent = if indent.len() * 2 > width {
        Vec::new()
    } else {
        indent
    };
    let mut rows: Vec<Vec<(char, Style)>> = Vec::new();
    let mut row: Vec<(char, Style)> = Vec::new();
    let mut used = 0;
    // The row holds nothing but its indentation.
    let mut fresh = true;
    let mut i = 0;
    while i < cells.len() {
        let gap_start = i;
        while i < cells.len() && cells[i].0 == ' ' {
            i += 1;
        }
        let gap = &cells[gap_start..i];
        let word_start = i;
        while i < cells.len() && cells[i].0 != ' ' {
            i += 1;
        }
        let word = &cells[word_start..i];
        let word_width: usize = word.iter().map(|(c, _)| char_width(*c)).sum();
        if word.is_empty() {
            // Trailing spaces: kept only on a line of nothing else.
            if rows.is_empty() && row.is_empty() {
                row.extend_from_slice(gap);
            }
            break;
        }
        let gap_width: usize = gap.iter().map(|(c, _)| char_width(*c)).sum();
        if !fresh && used + gap_width + word_width > width {
            rows.push(std::mem::take(&mut row));
            row.extend_from_slice(&indent);
            used = width_of(&indent);
            fresh = true;
        }
        if !fresh || rows.is_empty() {
            row.extend_from_slice(gap);
            used += gap_width;
        }
        for &(c, style) in word {
            let w = char_width(c);
            if used + w > width && used > width_of(&indent) {
                rows.push(std::mem::take(&mut row));
                row.extend_from_slice(&indent);
                used = width_of(&indent);
            }
            row.push((c, style));
            used += w;
        }
        fresh = false;
    }
    rows.push(row);
    rows.into_iter().map(|cells| spans_of(&cells)).collect()
}

fn width_of(cells: &[(char, Style)]) -> usize {
    cells.iter().map(|(c, _)| char_width(*c)).sum()
}

fn spans_of(cells: &[(char, Style)]) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut text = String::new();
    let mut current: Option<Style> = None;
    for &(c, style) in cells {
        if current != Some(style) {
            if let Some(s) = current {
                spans.push(Span::styled(std::mem::take(&mut text), s));
            }
            current = Some(style);
        }
        text.push(c);
    }
    if let Some(s) = current {
        spans.push(Span::styled(text, s));
    }
    Line::from(spans)
}

/// Every line of `lines` wrapped to `width`.
#[must_use]
pub fn wrap_all(lines: &[Line<'_>], width: usize) -> Vec<Line<'static>> {
    lines.iter().flat_map(|l| wrap(l, width)).collect()
}

/// `text` cut to `width` columns, with `…` when cut.
#[must_use]
pub fn truncated(text: &str, width: usize) -> String {
    if self::width(text) <= width {
        return text.to_owned();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars() {
        let w = char_width(c);
        if used + w + 1 > width {
            break;
        }
        out.push(c);
        used += w;
    }
    out.push('…');
    out
}

/// `spans` cut to `width` columns, with `…` when cut, each keeping its style.
#[must_use]
pub fn fit(spans: &[Span<'static>], width: usize) -> Vec<Span<'static>> {
    let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
    if self::width(&text) <= width {
        return spans.to_vec();
    }
    let cut = truncated(&text, width);
    let mut chars = cut.chars();
    let mut out = Vec::new();
    for span in spans {
        let piece: String = chars.by_ref().take(span.content.chars().count()).collect();
        if piece.is_empty() {
            break;
        }
        out.push(Span::styled(piece, span.style));
    }
    out
}

/// `spans` ending in `…` within `width` columns: what is cut to show more is cut.
#[must_use]
pub fn ellipsized(spans: &[Span<'static>], width: usize) -> Vec<Span<'static>> {
    let mut spans = spans.to_vec();
    spans.push(Span::raw("…"));
    fit(&spans, width)
}

/// How the focused (or selected) thing is drawn: bold on the highlight band, its colour kept.
#[must_use]
pub fn highlighted(style: Style) -> Style {
    style.patch(theme::style(Role::Selected))
}

/// `line` on the highlight band, its muted text lifted to grey so it still reads on the band.
#[must_use]
pub fn banded(line: Line<'static>) -> Line<'static> {
    let spans = line
        .spans
        .into_iter()
        .map(|s| {
            let lifted = if s.style.fg == Some(Color::DarkGray) {
                s.style.fg(Color::Gray)
            } else {
                s.style
            };
            Span::styled(s.content, highlighted(lifted))
        })
        .collect::<Vec<_>>();
    Line::from(spans).style(highlighted(line.style))
}

/// `style`, highlighted when the keyboard focus rests on `target`.
#[must_use]
pub fn focus_style(hits: &Hits, target: Target, style: Style) -> Style {
    if hits.focused(target) {
        highlighted(style)
    } else {
        style
    }
}

/// Tokens placed left to right and wrapped between tokens, remembering where each one landed.
#[derive(Debug)]
pub struct Flow {
    width: usize,
    rows: Vec<Vec<Span<'static>>>,
    used: usize,
    placed: Vec<(usize, usize, usize, Target)>,
}

impl Flow {
    #[must_use]
    pub fn new(width: u16) -> Self {
        Self {
            width: usize::from(width).max(1),
            rows: vec![Vec::new()],
            used: 0,
            placed: Vec::new(),
        }
    }

    /// Adds `span` after `gap` spaces (none at the start of a row), on a new row if it does not
    /// fit; a span wider than a row is cut. `target` is registered where it lands.
    /// The columns the current row has used.
    #[must_use]
    pub const fn used(&self) -> usize {
        self.used
    }

    pub fn token(&mut self, span: Span<'static>, gap: usize, target: Option<Target>) {
        let mut span = span;
        let w = width(&span.content);
        if self.used > 0 && self.used + gap + w > self.width {
            self.newline();
        }
        if self.used > 0 {
            self.push_raw(Span::raw(" ".repeat(gap)));
        }
        if w > self.width - self.used {
            span.content = truncated(&span.content, self.width - self.used).into();
        }
        let w = width(&span.content);
        if let Some(target) = target {
            self.placed
                .push((self.rows.len() - 1, self.used, w, target));
        }
        self.push_raw(span);
    }

    fn push_raw(&mut self, span: Span<'static>) {
        self.used += width(&span.content);
        if let Some(row) = self.rows.last_mut() {
            row.push(span);
        }
    }

    /// The columns left on the current row.
    #[must_use]
    pub const fn width_left(&self) -> usize {
        self.width - self.used
    }

    /// The columns of a row.
    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    /// Starts a new row.
    pub fn newline(&mut self) {
        self.rows.push(Vec::new());
        self.used = 0;
    }

    /// The rows used (a last row left empty by [`Flow::end_row`] does not count).
    #[must_use]
    pub const fn rows(&self) -> usize {
        if self.used == 0 && self.rows.len() > 1 {
            self.rows.len() - 1
        } else {
            self.rows.len()
        }
    }

    /// The lines, registering each placed target as drawn from `origin` (rows past its height
    /// are not drawn, so not registered).
    pub fn finish(mut self, origin: Rect, hits: &mut Hits) -> Vec<Line<'static>> {
        let rows = self.rows();
        self.rows.truncate(rows);
        for (row, x, w, target) in self.placed {
            if row < usize::from(origin.height) {
                let rect = Rect {
                    x: origin.x.saturating_add(coord(x)),
                    y: origin.y.saturating_add(coord(row)),
                    width: coord(w).min(origin.width.saturating_sub(coord(x))),
                    height: 1,
                };
                hits.push(rect, target);
            }
        }
        self.rows.into_iter().map(Line::from).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    fn text(lines: &[Line<'_>]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    #[test]
    fn wrapping_breaks_at_spaces_and_keeps_styles() {
        let red = Style::new().fg(Color::Red);
        let line = Line::from(vec![
            Span::raw("Deal "),
            Span::styled("12", red),
            Span::raw(" damage to all"),
        ]);
        let rows = wrap(&line, 10);
        assert_eq!(text(&rows), ["Deal 12", "damage to", "all"]);
        assert_eq!(rows[0].spans[1].style, red);
        assert_eq!(
            text(&wrap(&Line::from("abcdefghij"), 4)),
            ["abcd", "efgh", "ij"]
        );
        assert_eq!(text(&wrap(&Line::from(""), 4)), [""]);
    }

    #[test]
    fn continuation_rows_keep_the_indentation() {
        let rows = wrap(&Line::from("    one two three"), 12);
        assert_eq!(text(&rows), ["    one two", "    three"]);
    }

    #[test]
    fn tokens_wrap_whole_and_land_where_registered() {
        let mut flow = Flow::new(13);
        flow.token(Span::raw("Akabeko"), 0, Some(Target::Relic(0)));
        flow.token(Span::raw("Anchor"), 2, Some(Target::Relic(1)));
        flow.token(Span::raw("Vajra"), 2, Some(Target::Relic(2)));
        assert_eq!(flow.rows(), 2);
        let mut hits = Hits::new();
        let lines = flow.finish(Rect::new(5, 3, 13, 1), &mut hits);
        assert_eq!(text(&lines), ["Akabeko", "Anchor  Vajra"]);
        assert_eq!(hits.rect_of(Target::Relic(0)), Some(Rect::new(5, 3, 7, 1)));
        assert_eq!(hits.rect_of(Target::Relic(1)), None, "past the height");
    }

    #[test]
    fn truncation_marks_the_cut() {
        assert_eq!(truncated("Strike", 10), "Strike");
        assert_eq!(truncated("Strike+", 5), "Stri…");
    }
}
