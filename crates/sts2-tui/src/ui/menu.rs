//! The screens that are a choice: Neow and the ancients, events, rewards, card selections,
//! bundles, rest sites, treasure rooms, the shop and stub rooms. A title, the story or the
//! question in a few lines, then each choice as a full-width button with its number; a choice
//! that cannot be taken has none and is dim.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::hit::{Hits, Pane, Target, UiAction};
use crate::text;
use crate::ui::button::{self, Button};
use crate::ui::cards;
use crate::ui::column::Stack;
use crate::ui::theme::{Role, style};
use crate::ui::{flow, notice, scale, title};
use sts2_view::model::{self, Block, ButtonState, Key, Screen};
use sts2_view::rooms::changes_text;
use sts2_view::text::Rich;

/// The most rows of a choice's text under its name.
const DETAIL_ROWS: usize = 2;

/// Draws the decision's screen in `area`: the title, the text (cut to the rows the choices
/// leave), the choices (scrolled by the wheel when they do not fit), the line for an error or
/// the number typed, and the way on (Leave, Done) as the main button.
pub fn draw(frame: &mut Frame<'_>, area: Rect, app: &App, hits: &mut Hits) {
    if area.is_empty() {
        return;
    }
    let Some(model) = app.room_screen() else {
        return;
    };
    let (heading, lines) = model_text(&model);
    let notice = notice(app);
    let width = usize::from(area.width);
    let text = flow::wrap_all(&lines, width);
    let list_height = |choices: &Choices| {
        if choices.list.is_empty() {
            0
        } else {
            button::list_height(&choices.list, area.width, true)
        }
    };
    // The main button framed while there is room for it with everything else.
    let main_rows = |choices: &Choices, framed: bool| match (&choices.main, framed) {
        (None, _) => 0,
        (Some(_), true) => scale::primary_rows() + 1,
        (Some(_), false) => 2,
    };
    // Title, its blank, the blank under the text, the notice.
    let fixed = 3 + u16::from(notice.is_some()) + u16::from(!text.is_empty());
    let needed = |choices: &Choices| {
        fixed + list_height(choices) + main_rows(choices, true) + flow::coord(text.len())
    };
    // A big screen's card rows take two rows each while they fit with everything else.
    let choose = |tall: bool| model_choices(&model, tall);
    let mut choices = choose(scale::big());
    if scale::big() && needed(&choices) > area.height {
        choices = choose(false);
    }
    let list_height = list_height(&choices);
    let framed = needed(&choices) <= area.height;
    let fixed = fixed + main_rows(&choices, framed);
    // The question keeps a few rows however many choices there are: the choices scroll.
    let rows_for = |fixed: u16| {
        usize::from(area.height.saturating_sub(fixed + list_height))
            .max(3)
            .min(text.len())
    };
    // The story is cut only after the blank rows round it went: the words are what the player
    // chooses by, and nothing else on the screen reads them out.
    let cramped = rows_for(fixed) < text.len() && !text.is_empty();
    let fixed = if cramped { fixed - 2 } else { fixed };
    let text_rows = rows_for(fixed);
    // On a big screen, rows to spare widen the blank rows between the parts, a little.
    let spare = area
        .height
        .saturating_sub(fixed + list_height + flow::coord(text_rows));
    let gap = if scale::big() && spare >= 12 { 2 } else { 1 };
    let mut stack = Stack::new(area);
    if !cramped {
        stack.skip(gap);
    }
    title(frame, stack.take(1), &heading);
    if !cramped {
        stack.skip(gap);
    }
    if !text.is_empty() {
        let mut text = text;
        if text_rows < text.len() {
            text.truncate(text_rows);
            if let Some(last) = text.last_mut() {
                *last = Line::from(flow::ellipsized(&last.spans, width));
            }
        }
        frame.render_widget(
            Paragraph::new(text).style(Style::new().fg(Color::Gray)),
            stack.take(flow::coord(text_rows)),
        );
        stack.skip(gap);
    }
    let below = u16::from(notice.is_some()) + main_rows(&choices, framed);
    let list = stack.take(list_height.min(stack.left().saturating_sub(below)));
    hits.push(list, Target::Pane(Pane::Menu));
    let max = button::draw_list(frame, list, &choices.list, true, app.menu_scroll(), hits);
    hits.limit(Pane::Menu, 0, isize::try_from(max).unwrap_or(0));
    if let Some(footer) = &choices.footer {
        draw_footer(frame, list, footer);
    }
    if let Some(notice) = notice {
        frame.render_widget(Paragraph::new(notice), stack.take(1));
    }
    if let Some((key, label, action)) = &choices.main {
        stack.skip(
            gap.min(stack.left().saturating_sub(scale::primary_rows()))
                .max(1),
        );
        let rows = if framed { scale::primary_rows() } else { 1 };
        button::draw_primary(frame, stack.take(rows), key, label, *action, true, hits);
    }
    // Esc only where there is a screen to go back to.
    let nested = heading == "Choose a card"
        || choices.main.as_ref().is_some_and(|(key, ..)| key == "Esc")
        || lines.iter().any(|l| l.to_string().contains("Esc"));
    key_hints(frame, &stack, &choices, nested);
}

/// The key hints under a screen of choices.
fn key_hints(frame: &mut Frame<'_>, stack: &Stack, choices: &Choices, nested: bool) {
    let pick = button::pick_hint(
        choices
            .list
            .iter()
            .map(|b| b.key.as_str())
            .chain(choices.main.as_ref().map(|(key, ..)| key.as_str())),
    );
    let mut hint = vec!["↑↓ move", "Enter choose"];
    if !pick.is_empty() {
        hint.push(&pick);
    }
    if nested {
        hint.push("Esc back");
    }
    hint.extend(["? help", "Tab menu"]);
    crate::ui::hints(frame, stack, &hint.join(" · "));
}

/// The page line on the list's bottom edge, at the right.
fn draw_footer(frame: &mut Frame<'_>, list: Rect, footer: &str) {
    if list.height < 2 {
        return;
    }
    let footer = format!(" {footer} ");
    let width = flow::coord(flow::width(&footer)).min(list.width.saturating_sub(2));
    let at = Rect {
        x: list.right().saturating_sub(width + 1),
        y: list.bottom() - 1,
        width,
        height: 1,
    };
    frame.render_widget(Paragraph::new(Span::styled(footer, style(Role::Muted))), at);
}

/// The screen's title and the lines above its choices, from the model.
fn model_text(screen: &Screen) -> (String, Vec<Line<'static>>) {
    let heading = screen.title.as_ref().map_or_else(String::new, Rich::text);
    let lines = screen
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Text(lines) => Some(lines),
            _ => None,
        })
        .flatten()
        .map(|rich| Line::from(text::spans(rich)))
        .collect();
    (heading, lines)
}

/// The screen's choices from the model: the list rows (a card as a card row, two rows when
/// `tall`), the way on as the main button, and the page line.
fn model_choices(screen: &Screen, tall: bool) -> Choices {
    let mut choices = Choices {
        list: Vec::new(),
        main: None,
        footer: None,
    };
    let key = |row: &model::Button| row.key.map_or_else(String::new, key_label);
    let target = |row: &model::Button| {
        (row.id.0 < ROW_LIMIT).then_some(Target::Button(UiAction::Pick(row.id.0 as usize)))
    };
    if let Some(main) = &screen.primary {
        choices.main = Some((
            key(main),
            main.label.text(),
            UiAction::Pick(main.id.0 as usize),
        ));
    }
    for block in &screen.blocks {
        let Block::List { rows, page, .. } = block else {
            continue;
        };
        let faces: Vec<cards::Row> = rows
            .iter()
            .filter_map(|row| row.card.as_ref())
            .map(|face| {
                // An upgrade's row: only what it changes (`6 → 9`), so the row stays one line.
                let changes = face
                    .before
                    .as_ref()
                    .map(|now| text::spans(&changes_text(&now.text.text(), &face.text.text())));
                cards::Row {
                    cost: face.cost.clone(),
                    name: face.name.clone(),
                    kind: face.kind.clone(),
                    rarity: face.rarity.clone(),
                    effect: changes.clone().unwrap_or_else(|| text::spans(&face.effect)),
                    text: changes.unwrap_or_else(|| text::spans(&face.text)),
                    glow: face.glow,
                }
            })
            .collect();
        let mut card_lines = cards::spans(&faces, tall).into_iter();
        for (at, row) in rows.iter().enumerate() {
            let card_line = row
                .card
                .is_some()
                .then(|| card_lines.next().unwrap_or_default());
            if page.is_some_and(|p| at < p.first || at >= p.first + p.len) {
                continue;
            }
            let asking = row.state == ButtonState::Picked && row.confirm.is_some();
            let button = if asking {
                Button::label(key(row), row.label.text(), target(row))
            } else if let Some((line, expanded)) = card_line {
                let picked = row.state == ButtonState::Picked;
                Button::new(key(row), line, target(row))
                    .right(row.right.as_ref().map(text::spans).unwrap_or_default())
                    .selected(picked)
                    .more(true)
                    .dim(matches!(row.state, ButtonState::Dim(_)) && !picked)
                    .expanded(expanded)
                    .open(scale::wide() || tall)
            } else {
                Button::new(key(row), text::spans(&row.label), target(row))
                    .right(row.right.as_ref().map(text::spans).unwrap_or_default())
                    .detail(
                        row.detail
                            .iter()
                            .map(|d| Line::from(text::spans(d)))
                            .collect(),
                    )
                    .max_detail(DETAIL_ROWS)
                    .more(row.info.is_some() || !row.detail.is_empty())
                    .dim(matches!(row.state, ButtonState::Dim(_)))
            };
            choices.list.push(button);
        }
        // A list longer than a page shows the page the focus is on.
        if let Some(page) = page {
            let turn = match page.index {
                0 => "PgDn",
                p if p + 1 == page.count => "PgUp",
                _ => "PgUp/PgDn",
            };
            choices.footer = Some(format!("page {}/{} · {turn}", page.index + 1, page.count));
        }
    }
    choices
}

/// The row ids at and above this are rows that cannot be picked.
const ROW_LIMIT: u32 = 10_000;

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

/// A screen's choices: the list (one page of it when it is long), the way on (Leave, Done) as
/// the main button with its key, and the page line for the frame's bottom (`page 1/3 · PgDn`).
struct Choices {
    list: Vec<Button>,
    main: Option<(String, String, UiAction)>,
    footer: Option<String>,
}
