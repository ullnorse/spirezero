//! The screens before a run, drawn from `sts2-view`'s [`Screen`]: the main menu while a saved run
//! waits (Continue, New run, Settings, Quit) and the character select: one character at a time
//! (`◀ IRONCLAD ▶`, `←`/`→`, its number keys under it), the chosen one's text, the ascension
//! (`▾ Ascension 0 ▴`, `↑`/`↓`), the seed, and Start. Each control's arrows point the way its
//! keys turn it.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use sts2_core::Character;
use sts2_view::launch::ASCENSION;
use sts2_view::model::{Block, ButtonId, ButtonState, Key, Screen};
use sts2_view::text::Rich;

use crate::hit::{Hits, Pane, Target, UiAction};
use crate::shell::CharacterSelect;
use crate::text;
use crate::ui::button::{self, Button};
use crate::ui::column::{self, Stack};
use crate::ui::flow;
use crate::ui::scale;
use crate::ui::theme::{Role, style};

/// The action a row presses: the main menu's, or the select's.
const fn action(menu: bool, id: ButtonId) -> Option<UiAction> {
    if menu {
        return Some(match id.0 {
            1 => UiAction::Continue,
            2 => UiAction::ContinueEarlier,
            3 => UiAction::NewRun,
            4 => UiAction::Settings,
            5 => UiAction::Quit,
            _ => return None,
        });
    }
    match id {
        ButtonId(n @ 1..=5) => Some(UiAction::SelectCharacter(n as usize - 1)),
        sts2_view::launch::SEED => Some(UiAction::EditSeed),
        _ => None,
    }
}

/// A key as a row prints it.
fn key_label(key: Key) -> String {
    match key {
        Key::Digit(d) => d.to_string(),
        Key::Char(c) => c.to_string(),
        Key::Enter => "Enter".to_owned(),
        Key::Esc => "Esc".to_owned(),
        _ => String::new(),
    }
}

/// `rich` wrapped to the column, centred, from the top of `stack`; the rows it may take are
/// at most `rows`, the last cut with `…`. With `fixed` it takes all `rows` whatever its length,
/// so what follows it stays put.
fn note(frame: &mut Frame<'_>, stack: &mut Stack, lines: &[Rich], rows: usize, fixed: bool) {
    let width = usize::from(stack.rest().width);
    let lines: Vec<Line<'static>> = lines
        .iter()
        .map(|rich| Line::from(text::spans(rich)))
        .collect();
    let mut lines = flow::wrap_all(&lines, width);
    if lines.len() > rows {
        lines.truncate(rows);
        if let Some(last) = lines.last_mut() {
            *last = Line::from(flow::ellipsized(&last.spans, width));
        }
    }
    let lines: Vec<Line<'static>> = lines.into_iter().map(Line::centered).collect();
    let taken = if fixed { rows } else { lines.len() };
    let area = stack.take(flow::coord(taken));
    frame.render_widget(Paragraph::new(lines), area);
}

/// The rows the chosen character's text takes, and the ascension's rule under its stepper,
/// whatever the character or the level: switching either moves nothing below it.
const CHARACTER_ROWS: usize = 6;
const RULE_ROWS: usize = 2;

/// One list block's rows as buttons.
fn buttons(rows: &[sts2_view::model::Button], menu: bool) -> Vec<Button> {
    rows.iter()
        .map(|row| {
            let mut button = Button::new(
                row.key.map_or_else(String::new, key_label),
                text::spans(&row.label),
                action(menu, row.id).map(Target::Button),
            )
            .right(row.right.as_ref().map(text::spans).unwrap_or_default())
            .selected(!menu && row.state == ButtonState::Selected);
            if !row.detail.is_empty() {
                button = button
                    .detail(
                        row.detail
                            .iter()
                            .map(|d| Line::from(text::spans(d)))
                            .collect(),
                    )
                    .max_detail(2);
            }
            button
        })
        .collect()
}

/// Draws the main menu or the character select over the whole frame.
pub fn draw(frame: &mut Frame<'_>, select: &CharacterSelect, hits: &mut Hits) {
    hits.set_focus(select.focused());
    let menu = select.continue_focused();
    let screen = select.screen();
    let area = crate::ui::choice_column(frame.area(), true, false);
    let mut stack = Stack::new(area);
    stack.skip(1);
    let heading = if menu {
        "Slay the Spire 2".to_owned()
    } else {
        "New run".to_owned()
    };
    frame.render_widget(
        Paragraph::new(Span::styled(
            heading,
            Style::new().add_modifier(Modifier::BOLD),
        ))
        .centered(),
        stack.take(1),
    );
    let rule = stack.take(1);
    frame.render_widget(
        Paragraph::new(Span::styled(
            "─".repeat(usize::from(rule.width.min(24))),
            style(Role::Muted),
        ))
        .centered(),
        rule,
    );
    stack.skip(1);
    draw_blocks(frame, &mut stack, &screen, menu, hits);
}

/// What a list block of the select is: the characters, the ascension (and the rows after it),
/// or a plain list.
enum Part<'a> {
    Characters(&'a [sts2_view::model::Button]),
    Ascension(&'a sts2_view::model::Button, Vec<Button>),
    List(Vec<Button>),
}

impl<'a> Part<'a> {
    fn of(rows: &'a [sts2_view::model::Button], menu: bool) -> Self {
        match rows.first() {
            Some(first) if !menu && first.id == ButtonId(1) => Self::Characters(rows),
            Some(first) if !menu && first.id == ASCENSION => {
                Self::Ascension(first, buttons(&rows[1..], menu))
            }
            _ => Self::List(buttons(rows, menu)),
        }
    }

    /// The rows the part takes `width` wide, the blank after it included.
    fn height(&self, width: u16) -> u16 {
        match self {
            Self::Characters(_) => 3,
            Self::Ascension(_, rest) => {
                1 + flow::coord(RULE_ROWS) + button::list_height(rest, width, true) + 1
            }
            Self::List(list) => button::list_height(list, width, true) + 1,
        }
    }
}

fn draw_blocks(
    frame: &mut Frame<'_>,
    stack: &mut Stack,
    screen: &Screen,
    menu: bool,
    hits: &mut Hits,
) {
    let width = stack.rest().width;
    let parts: Vec<Part<'_>> = screen
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::List { rows, .. } => Some(Part::of(rows, menu)),
            _ => None,
        })
        .collect();
    // What the lists, the main button and the line under it take, and a blank between parts.
    let reserved: u16 = parts.iter().map(|p| p.height(width)).sum::<u16>()
        + screen
            .primary
            .as_ref()
            .map_or(0, |_| scale::primary_rows() + 1)
        + 1;
    let mut parts = parts.into_iter();
    for block in &screen.blocks {
        match block {
            Block::Text(lines) => {
                let budget = usize::from(stack.left().saturating_sub(reserved)).max(2);
                if menu {
                    note(frame, stack, lines, budget, false);
                } else {
                    let lines = without_name(lines);
                    note(frame, stack, &lines, budget.min(CHARACTER_ROWS), true);
                }
                stack.skip(1);
            }
            Block::List { .. } => {
                match parts.next() {
                    Some(Part::Characters(rows)) => draw_characters(frame, stack, rows, hits),
                    Some(Part::Ascension(row, rest)) => {
                        draw_ascension(frame, stack, row, hits);
                        if !rest.is_empty() {
                            let area = stack.take(button::list_height(&rest, width, true));
                            button::draw_list(frame, area, &rest, true, 0, hits);
                        }
                    }
                    Some(Part::List(list)) => {
                        let area = stack.take(button::list_height(&list, width, true));
                        button::draw_list(frame, area, &list, true, 0, hits);
                    }
                    None => {}
                }
                stack.skip(1);
            }
            _ => {}
        }
    }
    if let Some(primary) = &screen.primary {
        let editing = primary.label.text() == "Done";
        let action = if editing {
            UiAction::EditSeed
        } else {
            UiAction::Start
        };
        button::draw_primary(
            frame,
            stack.take(scale::primary_rows()),
            &primary.key.map_or_else(String::new, key_label),
            &primary.label.text(),
            action,
            true,
            hits,
        );
    }
    let line = screen.status.as_ref().map_or_else(
        || {
            let hint = if menu {
                "↑↓ move · Enter choose · ? help"
            } else {
                "←→ character · ↑↓ ascension · ? help"
            };
            Line::from(Span::styled(hint, style(Role::Muted)))
        },
        |status| Line::from(text::spans(status)),
    );
    let width = usize::from(stack.rest().width);
    let lines: Vec<Line<'static>> = flow::wrap(&line, width)
        .into_iter()
        .map(Line::centered)
        .collect();
    let rows = stack.take(flow::coord(lines.len()));
    frame.render_widget(Paragraph::new(lines), rows);
}

/// The chosen character's text without the name its first line opens with (`IRONCLAD  ♥ 80/80
/// ◉ 99` → `♥ 80/80  ◉ 99`): the carousel above shows the name already.
fn without_name(lines: &[Rich]) -> Vec<Rich> {
    let mut lines = lines.to_vec();
    if let Some(first) = lines.first_mut() {
        let named = first.0.first().is_some_and(|s| s.role == Role::Title);
        if named {
            let skip = first
                .0
                .iter()
                .skip(1)
                .take_while(|s| s.text.trim().is_empty())
                .count();
            first.0.drain(..=skip);
        }
    }
    lines
}

/// A character's colour on the select.
const fn character_color(character: Character) -> Color {
    match character {
        Character::Ironclad => Color::Red,
        Character::Silent => Color::Green,
        Character::Defect => Color::Cyan,
        Character::Regent => Color::Yellow,
        Character::Necrobinder => Color::Magenta,
    }
}

/// `back middle on` in the middle of `row`: the two arrow glyphs are buttons for their actions.
fn arrows(
    frame: &mut Frame<'_>,
    row: Rect,
    middle: Span<'static>,
    (back, on): ((&'static str, UiAction), (&'static str, UiAction)),
    hits: &mut Hits,
) {
    let width = flow::coord(flow::width(&middle.content));
    let text = column::middle(row, width);
    frame.render_widget(Paragraph::new(middle), text);
    for ((glyph, action), x) in [(back, text.x.saturating_sub(5)), (on, text.right() + 2)] {
        let rect = Rect { x, width: 3, ..row }.intersection(row);
        hits.push(rect, Target::Button(action));
        frame.render_widget(Paragraph::new(Span::styled(glyph, style(Role::Key))), rect);
    }
}

/// The chosen character as `◀ IRONCLAD ▶` in its colour (the arrows step through them), the
/// characters' numbers under it with the chosen one lit (each picks its character).
fn draw_characters(
    frame: &mut Frame<'_>,
    stack: &mut Stack,
    rows: &[sts2_view::model::Button],
    hits: &mut Hits,
) {
    let n = rows.len().max(1);
    let index = rows
        .iter()
        .position(|r| r.state == ButtonState::Selected)
        .unwrap_or(0);
    let name = rows.get(index).map(|r| r.label.text()).unwrap_or_default();
    let color = Character::from_name(&name).map_or(Color::Reset, character_color);
    arrows(
        frame,
        stack.take(1),
        Span::styled(
            name.to_uppercase(),
            Style::new().fg(color).add_modifier(Modifier::BOLD),
        ),
        (
            (" ◀ ", UiAction::SelectCharacter((index + n - 1) % n)),
            (" ▶ ", UiAction::SelectCharacter((index + 1) % n)),
        ),
        hits,
    );
    let keys = column::middle(stack.take(1), flow::coord(3 * n - 2));
    for (i, row) in rows.iter().enumerate() {
        let rect = Rect {
            x: keys.x + flow::coord(3 * i),
            width: 1,
            ..keys
        };
        hits.push(rect, Target::Button(UiAction::SelectCharacter(i)));
        let look = if i == index {
            Style::new()
                .fg(color)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            style(Role::Muted)
        };
        let key = row.key.map_or_else(|| (i + 1).to_string(), key_label);
        frame.render_widget(Paragraph::new(Span::styled(key, look)), rect);
    }
}

/// `▾ Ascension 2 ▴` (the ends and the wheel turn it, as `↓`/`↑` do) and the level's rule
/// under it, in [`RULE_ROWS`] rows at every level (blank at 0).
fn draw_ascension(
    frame: &mut Frame<'_>,
    stack: &mut Stack,
    row: &sts2_view::model::Button,
    hits: &mut Hits,
) {
    let line = stack.take(1);
    hits.push(line, Target::Pane(Pane::Ascension));
    arrows(
        frame,
        line,
        Span::styled(row.label.text(), style(Role::Key)),
        (
            (" ▾ ", UiAction::Ascension(false)),
            (" ▴ ", UiAction::Ascension(true)),
        ),
        hits,
    );
    note(frame, stack, &row.detail, RULE_ROWS, true);
}
