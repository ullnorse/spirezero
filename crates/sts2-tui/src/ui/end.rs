//! The end of a run: DEFEAT, VICTORY, or STOPPED where the engine has not ported what came
//! next; the story of the run in a few lines (what killed you, the rooms, the fights, the deck),
//! and the ways on as buttons. All of it is `sts2_view::rooms::end_screen`'s; this draws it.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use sts2_view::model::{Block, Screen};
use sts2_view::rooms::END_BUTTONS;

use crate::app::{App, end_action};
use crate::hit::{Hits, Target};
use crate::text;
use crate::ui::button::{self, Button};
use crate::ui::column::Stack;
use crate::ui::flow;

/// Draws the end screen in `area`.
pub fn draw(frame: &mut Frame<'_>, area: Rect, app: &App, hits: &mut Hits) {
    let Some(screen) = app.end_screen() else {
        return;
    };
    draw_screen(frame, area, &screen, hits);
}

fn draw_screen(frame: &mut Frame<'_>, area: Rect, screen: &Screen, hits: &mut Hits) {
    let width = usize::from(area.width);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut buttons: Vec<Button> = Vec::new();
    for block in &screen.blocks {
        match block {
            Block::Text(notes) => lines.extend(notes.iter().flat_map(|note| {
                flow::wrap(&Line::from(text::spans(note)), width)
                    .into_iter()
                    .map(|l| l.style(Style::new().fg(Color::Gray)).centered())
            })),
            Block::List { rows, .. } => {
                buttons.extend(rows.iter().zip(END_BUTTONS).map(|(row, (_, _, action))| {
                    let key = row.key.map_or_else(String::new, |k| match k {
                        sts2_view::model::Key::Char(c) => c.to_string(),
                        _ => String::new(),
                    });
                    Button::label(
                        key,
                        row.label.text(),
                        Some(Target::Button(end_action(action))),
                    )
                    .right(row.right.as_ref().map(text::spans).unwrap_or_default())
                }));
            }
            _ => {}
        }
    }
    let mut stack = Stack::new(area);
    stack.skip(1);
    let head = stack.take(1);
    let word = screen.title.as_ref().map(text::spans).unwrap_or_default();
    frame.render_widget(Paragraph::new(Line::from(word)).centered(), head);
    if let Some(aside) = &screen.aside {
        frame.render_widget(
            Paragraph::new(Span::styled(aside.text(), Style::new().fg(Color::Gray))).centered(),
            stack.take(1),
        );
    }
    stack.skip(1);
    let list_rows = button::list_height(&buttons, area.width, true);
    // The story keeps its rows while the buttons fit under it; short of rows it is cut.
    let text_rows = flow::coord(lines.len()).min(stack.left().saturating_sub(list_rows + 1));
    let text = stack.take(text_rows);
    frame.render_widget(Paragraph::new(lines), text);
    stack.skip(1);
    let list = stack.take(list_rows);
    button::draw_list(frame, list, &buttons, true, 0, hits);
    crate::ui::hints(frame, &stack, "↑↓ move · Enter choose · row keys pick");
}
