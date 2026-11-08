//! The help overlay (`?`): every key and mouse action of the screen under it, listed from the
//! same binding layers the input handler reads ([`Shell::help_layers`]), scrolled by the arrows.

use ratatui::Frame;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::hit::{Hits, Pane, Target, UiAction};
use crate::keys::{self, Binding, Keys};
use crate::shell::{Mode, Shell};
use crate::ui::{flow, page};

/// The help's sections: a title and its `(keys, what they do)` rows.
#[must_use]
pub fn sections(shell: &Shell) -> Vec<(&'static str, Vec<(String, &'static str)>)> {
    let mut sections: Vec<(&'static str, Vec<(String, &'static str)>)> = Vec::new();
    for layer in shell.help_layers() {
        let rows: Vec<(String, &'static str)> = layer
            .bindings
            .iter()
            .filter(|b| !b.label.is_empty())
            .map(|b| (label(shell, b), b.help))
            .collect();
        if rows.is_empty() {
            continue;
        }
        // Layers of the same name (the saved run's two) share a section.
        match sections.iter_mut().find(|(t, _)| *t == layer.title) {
            Some((_, existing)) => existing.extend(rows),
            None => sections.push((layer.title, rows)),
        }
    }
    if let Mode::Play(app) = shell.mode() {
        for (title, rows) in app.fight_help() {
            let rows: Vec<(String, &'static str)> =
                rows.iter().map(|&(k, what)| (k.to_owned(), what)).collect();
            match sections.iter_mut().find(|(t, _)| *t == title) {
                Some((_, existing)) => existing.extend(rows),
                None => sections.push((title, rows)),
            }
        }
    }
    if shell.settings().mouse {
        let gestures = match shell.mode() {
            Mode::Select(_) => keys::MOUSE_SELECT,
            Mode::Play(_) => keys::MOUSE_RUN,
        };
        sections.push((
            "Mouse",
            gestures
                .iter()
                .map(|&(g, what)| (g.to_owned(), what))
                .collect(),
        ));
    }
    sections
}

/// How the help writes `binding`'s keys.
fn label(shell: &Shell, binding: &Binding) -> String {
    match binding.keys {
        Keys::Boss => shell.settings().boss_key_names(),
        _ => binding.label.to_owned(),
    }
}

/// The widest a column of the grid gets.
const COLUMN: usize = 68;

/// Draws the help on a page of its own, scrolled `scroll` rows.
///
/// The arrows, PgUp/PgDn and the wheel move it; any other key or a click closes it. Each
/// section's keys stand one per line; the rule's end says how much more there is.
pub fn draw(frame: &mut Frame<'_>, shell: &Shell, scroll: usize, hits: &mut Hits) {
    let area = page(frame, "Keys and mouse", UiAction::Close, hits);
    hits.push(area, Target::Button(UiAction::Close));
    hits.push(area, Target::Pane(Pane::Help));
    let width = usize::from(area.width);
    let sections = sections(shell);
    let footer = Line::from(Span::styled(
        "Every other key is printed on its button. ↑↓ scroll · any other key or a click closes \
         this · Ctrl-C quits anywhere; the run is saved after every action.",
        Style::new().fg(Color::DarkGray),
    ));
    let mut lines = flow::wrap_all(&grid(&sections, width), width);
    lines.extend(flow::wrap(&footer, width));
    let height = usize::from(area.height);
    let max_scroll = lines.len().saturating_sub(height);
    let top = scroll.min(max_scroll);
    hits.limit(Pane::Help, 0, isize::try_from(max_scroll).unwrap_or(0));
    let more = if max_scroll > top {
        format!(" ▼ {} more ", max_scroll - top)
    } else if top > 0 {
        format!(" ▲ {top} more ")
    } else {
        String::new()
    };
    if !more.is_empty() {
        let w = flow::coord(flow::width(&more));
        let at = ratatui::layout::Rect {
            x: area.right().saturating_sub(w),
            y: area.y.saturating_sub(1),
            width: w.min(area.width),
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(Span::styled(more, Style::new().fg(Color::Yellow))),
            at,
        );
    }
    let visible: Vec<Line<'static>> = lines.into_iter().skip(top).take(height).collect();
    frame.render_widget(Paragraph::new(visible), area);
}

type Sections = [(&'static str, Vec<(String, &'static str)>)];

fn title(title: &'static str) -> Line<'static> {
    Line::from(Span::styled(
        title,
        Style::new().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    ))
}

/// Each section's keys one per line, in as many columns as fit (one for a section with a
/// long line).
fn grid(sections: &Sections, width: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (name, rows) in sections {
        lines.push(title(name));
        let key_width = rows.iter().map(|(k, _)| flow::width(k)).max().unwrap_or(0);
        let widest = rows.iter().map(|(_, w)| flow::width(w)).max().unwrap_or(0) + key_width + 4;
        let columns = if widest > COLUMN {
            1
        } else {
            (width / widest.max(1)).clamp(1, 3)
        };
        let cell = width / columns;
        for chunk in rows.chunks(columns) {
            let mut spans = Vec::new();
            for (keys, what) in chunk {
                let pad = key_width.saturating_sub(flow::width(keys));
                spans.push(Span::styled(
                    format!(" {keys}{}  ", " ".repeat(pad)),
                    Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    (*what).to_owned(),
                    Style::new().fg(Color::Gray),
                ));
                if columns > 1 {
                    let used = key_width + 3 + flow::width(what);
                    spans.push(Span::raw(" ".repeat(cell.saturating_sub(used))));
                }
            }
            lines.push(Line::from(spans));
        }
    }
    lines
}
