//! The sheets over a screen, drawn from `sts2-view`'s [`Screen::sheet`]: the `≡` menu, the
//! lists (deck, relics, info, piles, log, rooms visited, the enlarged map) as a scrollable page,
//! and the inspect overlay (the TUI's own text for one thing).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use sts2_core::run::RunDecision;
use sts2_view::details;
use sts2_view::model::{Block, Button, ButtonState, DetailSheet, Key, MenuSheet, Sheet};
use sts2_view::sheets::Kind;

use crate::app::{App, Inspect, Overlay};
use crate::hit::{Hits, Pane, Target, UiAction};
use crate::text;
use crate::ui::button::{self, Button as Row};
use crate::ui::column::Stack;
use crate::ui::theme::{Role, style};
use crate::ui::{flow, map, page};

/// Draws the app's open overlay, if any.
pub fn draw(frame: &mut Frame<'_>, app: &App, hits: &mut Hits) {
    if let Some(Overlay::Inspect(what)) = app.overlay() {
        inspect(frame, app, what, hits);
        return;
    }
    let Some(screen) = app.sheet_screen() else {
        return;
    };
    let kind = app.sheet_kind();
    match &screen.sheet {
        Some(Sheet::Menu(menu)) => draw_menu(frame, app, menu, hits),
        Some(Sheet::Detail(sheet)) => {
            let grid = screen.blocks.iter().find_map(|b| match b {
                Block::Map(grid) => Some(grid),
                _ => None,
            });
            list(frame, app, kind, sheet, grid, hits);
        }
        _ => {}
    }
}

/// One thing in full: a card, an enemy or a menu entry (`i` then a number), from
/// `sts2_view::details`.
fn inspect(frame: &mut Frame<'_>, app: &App, what: Inspect, hits: &mut Hits) {
    let Some(sheet) = detail_of(app, what) else {
        return;
    };
    let lines = detail_lines(&sheet);
    if side_panel(frame, app, &sheet.title.text(), &lines, Some(hits)) {
        return;
    }
    popup(
        frame,
        &sheet.title.text(),
        &lines,
        hits,
        Scroll::FromTop(app.overlay_scroll()),
    );
}

/// The detail sheet of `what`, from the model's detail builders.
fn detail_of(app: &App, what: Inspect) -> Option<DetailSheet> {
    let describer = app.describer();
    Some(match (what, app.combat()) {
        (Inspect::Card(id), Some(c)) => details::card(describer, c.state(), id),
        (Inspect::Enemy(id), Some(c)) => details::enemy(describer, c, id),
        (Inspect::Potion(slot), _) => {
            let id = app.run().potions().get(slot).copied().flatten()?;
            let text = describer.potion(id).unwrap_or_default();
            details::text_of(
                &sts2_view::names::potion(id),
                &sts2_view::text::lines(&text, Role::Plain),
            )
        }
        (Inspect::Relic(i), _) => {
            let relic = app.run().state().relics.get(i)?;
            let text = describer
                .owned_relic(relic.kind, &app.relic_data(i))
                .unwrap_or_default();
            details::text_of(
                relic.kind.name(),
                &sts2_view::text::lines(&text, Role::Plain),
            )
        }
        (Inspect::Item(n), _) if matches!(app.run().pending_decision(), RunDecision::Map(_)) => {
            let grid = app.map_grid();
            let cell = grid
                .cells
                .iter()
                .find(|c| c.button.is_some_and(|b| b.0 as usize == n))?;
            details::map_room(cell, &grid.kinds)
        }
        (Inspect::Item(n), _) => {
            let items = app.menu();
            let item = items.into_iter().filter(|i| i.enabled).nth(n - 1)?;
            match crate::menu::item_card(app.run(), &item.action) {
                Some(card) => details::run_card(describer, card),
                None => details::text_of(
                    &item.label,
                    &item
                        .detail
                        .as_deref()
                        .map_or_else(Vec::new, |d| sts2_view::text::lines(d, Role::Plain)),
                ),
            }
        }
        _ => return None,
    })
}

fn detail_lines(sheet: &DetailSheet) -> Vec<Line<'static>> {
    sheet
        .lines
        .iter()
        .map(|rich| Line::from(text::spans(rich)))
        .collect()
}

/// What `target` has details for, if anything.
fn inspect_of(app: &App, target: Target) -> Option<Inspect> {
    let fight = app.combat();
    match (target, fight) {
        (Target::Card(id) | Target::Choice(_, id), Some(_)) => Some(Inspect::Card(id)),
        (Target::Creature(id), Some(c)) if c.state().enemies.contains(&id) => {
            Some(Inspect::Enemy(id))
        }
        (Target::Button(UiAction::Pick(n)), None) => Some(Inspect::Item(n)),
        (Target::Potion(slot), _) => Some(Inspect::Potion(slot)),
        (Target::Relic(i), _) => Some(Inspect::Relic(i)),
        _ => None,
    }
}

/// The wide layout's panel, fed by the focus (or by `hover`, which never moves the focus): the
/// details of the row the keyboard rests on. Nothing when a sheet is open or there is no room.
pub fn focus_panel(frame: &mut Frame<'_>, app: &App, hover: Option<Target>) {
    if app.overlay().is_some() {
        return;
    }
    let target = hover
        .filter(|&t| inspect_of(app, t).is_some())
        .or_else(|| app.focused());
    let Some(sheet) = target
        .and_then(|t| inspect_of(app, t))
        .and_then(|what| detail_of(app, what))
    else {
        return;
    };
    // A row with nothing more to say than its own name has no panel to repeat it.
    if sheet.lines.is_empty() {
        return;
    }
    side_panel(frame, app, &sheet.title.text(), &detail_lines(&sheet), None);
}

/// The fewest cells the wide layout's panel is drawn in.
const PANEL_WIDTH: u16 = 32;

/// On a terminal wide enough to leave room beside the column, the detail as a panel on the right
/// (the screen stays in view and reachable); `false` when there is no such room.
fn side_panel(
    frame: &mut Frame<'_>,
    app: &App,
    title: &str,
    lines: &[Line<'_>],
    hits: Option<&mut Hits>,
) -> bool {
    let full = frame.area();
    let column = crate::ui::body_column(full, app);
    let x = column.right() + 2;
    let width = full.right().saturating_sub(x + 1).min(60);
    if width < PANEL_WIDTH || full.height < 10 {
        return false;
    }
    let panel = Rect {
        x,
        y: full.y + 1,
        width,
        height: full.height - 2,
    };
    frame.render_widget(Clear, panel);
    if let Some(hits) = hits {
        hits.push(panel, Target::Button(UiAction::Back));
    }
    let mut stack = Stack::new(panel);
    frame.render_widget(
        Paragraph::new(Span::styled(
            flow::truncated(title, usize::from(width.saturating_sub(4))),
            Style::new().add_modifier(Modifier::BOLD),
        )),
        stack.take(1),
    );
    let rule = stack.take(1);
    frame.render_widget(
        Paragraph::new(Span::styled(
            "─".repeat(usize::from(rule.width)),
            style(Role::Muted),
        )),
        rule,
    );
    let rows: Vec<Line<'static>> = flow::wrap_all(lines, usize::from(width));
    let rest = stack.rest();
    frame.render_widget(
        Paragraph::new(
            rows.into_iter()
                .take(usize::from(rest.height))
                .collect::<Vec<_>>(),
        ),
        rest,
    );
    true
}

/// A list sheet (and the map's grid above its legend) as a scrollable page.
fn list(
    frame: &mut Frame<'_>,
    app: &App,
    kind: Option<Kind>,
    sheet: &DetailSheet,
    grid: Option<&sts2_view::model::MapGrid>,
    hits: &mut Hits,
) {
    let (mut lines, rows) = grid.map_or_else(Default::default, |grid| {
        let (lines, rows) = map::full_lines(grid, hits);
        (lines, Some(rows))
    });
    lines.extend(sheet.lines.iter().map(|rich| Line::from(text::spans(rich))));
    let scroll = if kind == Some(Kind::Log) {
        Scroll::FromBottom(app.log_scroll())
    } else {
        Scroll::FromTop(app.sheet_scroll())
    };
    let (inner, top) = popup(frame, &sheet.title.text(), &lines, hits, scroll);
    // The reachable nodes can be clicked here too.
    if let (Some(grid), Some(rows)) = (grid, rows) {
        map::register_nodes(hits, grid, inner, top, &rows, map::Layout::COMPACT);
    }
}

/// How far a popup is scrolled: rows from its top (a list), or from its bottom (the log), each
/// with the pane the wheel scrolls.
#[derive(Clone, Copy)]
enum Scroll {
    FromTop(usize),
    FromBottom(usize),
}

/// A scrollable popup listing `lines` over a backdrop that closes it when clicked. Returns
/// the popup's inside and how many rows it is scrolled. The rule's end says how much more
/// there is.
fn popup(
    frame: &mut Frame<'_>,
    title: &str,
    lines: &[Line<'_>],
    hits: &mut Hits,
    scroll: Scroll,
) -> (Rect, usize) {
    let inner = page(frame, title, UiAction::Back, hits);
    let rows = flow::wrap_all(lines, usize::from(inner.width));
    let max_scroll = rows.len().saturating_sub(usize::from(inner.height));
    let (pane, top) = match scroll {
        Scroll::FromTop(n) => (Pane::Overlay, n.min(max_scroll)),
        Scroll::FromBottom(n) => (Pane::Log, max_scroll - n.min(max_scroll)),
    };
    hits.limit(pane, 0, isize::try_from(max_scroll).unwrap_or(0));
    hits.push(inner, Target::Pane(pane));
    let more = if max_scroll > top {
        format!(" ▼ {} more ", max_scroll - top)
    } else if top > 0 {
        format!(" ▲ {top} more ")
    } else {
        String::new()
    };
    if !more.is_empty() {
        let width = flow::coord(flow::width(&more));
        let at = Rect {
            x: inner.right().saturating_sub(width),
            y: inner.y.saturating_sub(1),
            width: width.min(inner.width),
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(Span::styled(more, Style::new().fg(Color::Yellow))),
            at,
        );
    }
    let visible: Vec<Line<'static>> = rows
        .into_iter()
        .skip(top)
        .take(usize::from(inner.height))
        .collect();
    frame.render_widget(Paragraph::new(visible), inner);
    (inner, top)
}

/// The key a row prints.
fn key_text(key: Option<Key>) -> String {
    match key {
        Some(Key::Char(c)) => c.to_string(),
        Some(Key::Digit(d)) => d.to_string(),
        _ => String::new(),
    }
}

/// The menu behind the top bar's `Tab ≡`: each entry with the key that opens it from any
/// screen (said under the list, so the keys can be learnt), the arrows and Enter too.
fn draw_menu(frame: &mut Frame<'_>, app: &App, sheet: &MenuSheet, hits: &mut Hits) {
    let actions: Vec<UiAction> = app.menu_rows().into_iter().map(|(a, _)| a).collect();
    let buttons: Vec<Row> = sheet
        .rows
        .iter()
        .zip(&actions)
        .map(|(row, action)| menu_row(row, *action))
        .collect();
    let mut stack = Stack::new(page(frame, &sheet.title.text(), UiAction::Back, hits));
    stack.skip(1);
    let list = stack.take(button::list_height(&buttons, stack.rest().width, true));
    button::draw_list(frame, list, &buttons, true, 0, hits);
    stack.skip(1);
    let note = flow::wrap(
        &Line::from(Span::styled(
            "Each key opens its list from any screen, without this menu.",
            style(Role::Muted),
        )),
        usize::from(stack.rest().width),
    );
    let rows = stack.take(flow::coord(note.len()));
    frame.render_widget(Paragraph::new(note), rows);
}

/// A menu row on one line: a throw-away row leaves its potion's text to the potions' list
/// (`i`), so the menu has no rules between rows and fits a small screen.
fn menu_row(row: &Button, action: UiAction) -> Row {
    let enabled = row.state == ButtonState::Enabled;
    Row::label(
        key_text(row.key),
        row.label.text(),
        enabled.then_some(Target::Button(action)),
    )
    .dim(!enabled)
}
