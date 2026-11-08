//! The settings (`o`): `sts2-view`'s settings page, one row per setting, changed with the keys
//! or a click and saved at once, on a page of its own.

use ratatui::Frame;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Wrap};
use sts2_view::launch::{SettingRow, settings_screen};
use sts2_view::model::{Block, ButtonState};
use sts2_view::text::Rich;

use crate::hit::{Hits, Target, UiAction};
use crate::keys;
use crate::settings::Row;
use crate::shell::Shell;
use crate::text;
use crate::ui::button::{self, Button};
use crate::ui::column::Stack;
use crate::ui::page;

/// Draws the settings with row `selected` highlighted.
pub fn draw(frame: &mut Frame<'_>, shell: &Shell, selected: usize, hits: &mut Hits) {
    let settings = shell.settings();
    let rows: Vec<SettingRow> = Row::ALL
        .iter()
        .map(|row| SettingRow {
            label: row.label().to_owned(),
            value: settings.value(*row).to_owned(),
            help: row.help().to_owned(),
        })
        .collect();
    let mut notes = vec![format!(
        "Boss keys: {} (set boss_keys in the file)",
        settings.boss_key_names()
    )];
    if let Some(session) = shell.session() {
        notes.push(format!("File: {}", session.dirs().settings().display()));
    }
    let keys: Vec<String> = keys::SETTINGS
        .bindings
        .iter()
        .filter(|b| !b.label.is_empty())
        .map(|b| format!("{} {}", b.label, b.help))
        .collect();
    notes.push(format!("{} · click: next value", keys.join(" · ")));
    let screen = settings_screen(&rows, selected, &notes);
    let title = screen.title.as_ref().map_or_else(String::new, Rich::text);
    let mut stack = Stack::new(page(frame, &title, UiAction::Close, hits));
    stack.skip(1);
    for block in &screen.blocks {
        match block {
            Block::List { rows, .. } => {
                let buttons: Vec<Button> = rows
                    .iter()
                    .map(|row| {
                        Button::new(
                            "",
                            text::spans(&row.label),
                            Some(Target::Button(UiAction::Setting(row.id.0 as usize))),
                        )
                        .right(row.right.as_ref().map(text::spans).unwrap_or_default())
                        .selected(row.state == ButtonState::Selected)
                    })
                    .collect();
                let list = stack.take(button::list_height(&buttons, stack.rest().width, true));
                button::draw_list(frame, list, &buttons, true, 0, hits);
                stack.skip(1);
            }
            Block::Text(lines) => {
                let lines: Vec<Line<'static>> = lines
                    .iter()
                    .map(|rich| Line::from(text::spans(rich)))
                    .collect();
                frame.render_widget(
                    Paragraph::new(lines)
                        .style(Style::new().fg(Color::Gray))
                        .wrap(Wrap { trim: true }),
                    stack.rest(),
                );
            }
            _ => {}
        }
    }
}
