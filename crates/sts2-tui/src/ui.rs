//! Drawing. Reads [`App`] and the engine's public state; never mutates either.
//!
//! Each frame also registers where its clickable things are ([`Hits`]), which the mouse handling
//! reads back, and highlights the keyboard focus the same way on every screen (`▸` and the
//! highlight band).
//!
//! Every screen is one narrow column ([`column`]), like a phone game: the top bar, then the
//! one decision of the screen with its choices as full-width buttons. Everything else (the
//! deck, the relics, the log, the map from a fight, the help) is one key or the `≡` menu away.

mod boss;
pub mod button;
mod cards;
pub mod column;
pub(crate) mod combat;
mod end;
pub(crate) mod flow;
pub mod glyph;
pub mod help;
mod launch;
mod map;
mod menu;
pub mod motion;
pub mod scale;
mod settings;
mod sheet;
pub mod theme;
mod top;

use ratatui::backend::Backend;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Clear, Paragraph};
use ratatui::{Frame, Terminal};
use sts2_core::run::RunDecision;

use crate::app::App;
use crate::hit::{Hits, Target, UiAction};
use crate::shell::{Mode, Panel, Shell};
use crate::ui::column::Stack;
use crate::ui::theme::{Role, style};

/// Draws whichever screen the shell is on.
pub fn draw_shell(frame: &mut Frame<'_>, shell: &Shell) {
    draw_shell_hits(frame, shell);
}

/// [`draw_shell`], returning where the clickable things were drawn.
pub fn draw_shell_hits(frame: &mut Frame<'_>, shell: &Shell) -> Hits {
    let mut hits = Hits::new();
    if shell.boss() {
        boss::draw(frame, shell.settings().boss_view);
        return hits;
    }
    theme::set_palette(shell.palette());
    glyph::set(shell.settings().glyphs);
    scale::set(frame.area());
    match shell.mode() {
        Mode::Select(select) => launch::draw(frame, select, &mut hits),
        Mode::Play(app) => draw_hovered(frame, app, shell.hover(), &mut hits),
    }
    match shell.panel() {
        Some(Panel::Help(scroll)) => help::draw(frame, shell, scroll, &mut hits),
        Some(Panel::Settings(row)) => settings::draw(frame, shell, row, &mut hits),
        None => {}
    }
    theme::apply(frame.buffer_mut());
    glyph::apply(frame.buffer_mut());
    hits
}

/// Draws the shell into `terminal` and keeps the frame's regions in the shell for the mouse.
///
/// # Errors
/// If the terminal cannot be drawn to.
pub fn render<B: Backend>(terminal: &mut Terminal<B>, shell: &mut Shell) -> Result<(), B::Error> {
    let mut hits = Hits::default();
    terminal.draw(|frame| hits = draw_shell_hits(frame, shell))?;
    shell.set_hits(hits);
    Ok(())
}

/// Draws the whole run screen.
pub fn draw(frame: &mut Frame<'_>, app: &App) {
    draw_with(frame, app, &mut Hits::new());
}

/// Draws the whole run screen, registering everything clickable in `hits`, the keyboard
/// focus highlighted.
///
/// The top bar, the decision's screen under it, and any overlay. A button pressed once that
/// cannot be undone shows its question in its own label (`App::confirm_row`); nothing pops up.
pub fn draw_with(frame: &mut Frame<'_>, app: &App, hits: &mut Hits) {
    draw_hovered(frame, app, None, hits);
}

/// [`draw_with`], the wide layout's panel following the mouse over `hover` as well as the focus.
pub fn draw_hovered(frame: &mut Frame<'_>, app: &App, hover: Option<Target>, hits: &mut Hits) {
    scale::set(frame.area());
    hits.set_focus(app.focused());
    let mut stack = Stack::new(body_column(frame.area(), app));
    top::draw(frame, &mut stack, &app.top_bar(), hits);
    let body = stack.rest();
    match app.run().pending_decision() {
        RunDecision::GameOver | RunDecision::Victory | RunDecision::Stopped(_) => {
            end::draw(frame, body, app, hits);
        }
        RunDecision::Combat(_) if app.combat().is_some() => combat::draw(frame, body, app, hits),
        RunDecision::Map(_) => map::draw(frame, body, app, hits),
        _ => menu::draw(frame, body, app, hits),
    }
    sheet::draw(frame, app, hits);
    sheet::focus_panel(frame, app, hover);
}

/// The column of the run screen `app` shows in `area`.
pub(crate) fn body_column(area: Rect, app: &App) -> Rect {
    // A fight uses the whole column; the map does too, except where there is room for the
    // panel that tells what the room under the highlight leads to.
    let (whole, panel) = match app.run().pending_decision() {
        RunDecision::Combat(_) => (true, false),
        RunDecision::Map(_) => (area.width < PANEL_FROM, true),
        RunDecision::GameOver | RunDecision::Victory | RunDecision::Stopped(_) => (false, false),
        _ => (false, true),
    };
    choice_column(area, !whole, panel)
}

/// The widest a screen of plain choices gets on a big terminal: the labels and what stands at
/// their right (a price, a count) stay together instead of a hand apart.
const CHOICE_WIDTH: u16 = 100;

/// The column of a screen: narrowed to [`CHOICE_WIDTH`] for a screen of choices on a big terminal
/// (with `panel`, a screen that has the detail panel beside it).
fn choice_column(area: Rect, choices: bool, panel: bool) -> Rect {
    let column = column::of(area);
    if !(choices && scale::big()) {
        return column;
    }
    // From [`PANEL_FROM`] cells wide the column makes room at its right for the detail panel
    // of the focused row (`sheet::focus_panel`), so the panel is there at 160 columns too: the
    // column and the panel stand side by side, centred together.
    if panel && area.width >= PANEL_FROM {
        let region = column.width.min(CHOICE_WIDTH + PANEL_ROOM);
        let width = CHOICE_WIDTH.min(region - PANEL_ROOM);
        return Rect {
            x: column.x + (column.width - region) / 2,
            width,
            ..column
        };
    }
    column::middle(column, CHOICE_WIDTH)
}

/// The terminal width from which a screen of choices leaves room for the detail panel.
const PANEL_FROM: u16 = 150;
/// The cells the panel and the gap before it take beside the choices.
const PANEL_ROOM: u16 = 46;

/// Draws the run screen with the fight shown as `screen` (a model built by hand), for tests
/// of the renderer at sizes and fights the engine does not easily make.
pub fn draw_fight_screen(frame: &mut Frame<'_>, app: &App, screen: &sts2_view::model::Screen) {
    let mut hits = Hits::new();
    scale::set(frame.area());
    hits.set_focus(app.focused());
    let mut stack = Stack::new(column::of(frame.area()));
    top::draw(frame, &mut stack, &app.top_bar(), &mut hits);
    combat::draw_screen(frame, stack.rest(), app, screen, &mut hits);
    theme::apply(frame.buffer_mut());
    glyph::apply(frame.buffer_mut());
}

/// The line under a screen's choices: an error (for one frame) or the number typed so far.
pub(crate) fn notice(app: &App) -> Option<Span<'static>> {
    if let Some(err) = app.error() {
        return Some(Span::styled(format!("✗ {err}"), style(Role::Lethal)));
    }
    let text = match app.input() {
        _ if !app.number().is_empty() => format!("> {}   Enter · Backspace", app.number()),
        _ => {
            return app
                .hint()
                .map(|hint| Span::styled(hint.to_owned(), style(Role::Heal)));
        }
    };
    Some(Span::styled(text, style(Role::Key)))
}

/// A page over the whole screen: everything under it cleared and out of reach, the column
/// holding `heading` with its close button `✕` (`close`, which a click outside the column
/// does too) and a rule. Returns the rows under the rule.
pub(crate) fn page(frame: &mut Frame<'_>, heading: &str, close: UiAction, hits: &mut Hits) -> Rect {
    let full = frame.area();
    let area = choice_column(full, true, false);
    frame.render_widget(Clear, full);
    hits.modal(full, area, Target::Button(close));
    let mut stack = Stack::new(area);
    let head = stack.take(1);
    frame.render_widget(
        Paragraph::new(Span::styled(
            flow::truncated(heading, usize::from(head.width.saturating_sub(4))),
            Style::new().add_modifier(Modifier::BOLD),
        )),
        head,
    );
    let target = Target::Button(close);
    let button = Rect {
        x: head.right().saturating_sub(3),
        width: head.width.min(3),
        ..head
    };
    hits.push(button, target);
    frame.render_widget(
        Paragraph::new(Span::styled(
            " ✕ ",
            Style::new().add_modifier(Modifier::BOLD),
        )),
        button,
    );
    let rule = stack.take(1);
    frame.render_widget(
        Paragraph::new(Span::styled(
            "─".repeat(usize::from(rule.width)),
            style(Role::Muted),
        )),
        rule,
    );
    stack.rest()
}

/// The key hints of a screen, dim, on the last row of `stack` when it has a row to spare
/// after a blank one: `↑↓ move · Enter choose · Esc back · ? help`, the last hints left out
/// while the row is too narrow for them all (never a cut hint).
pub(crate) fn hints(frame: &mut Frame<'_>, stack: &Stack, text: &str) {
    if stack.left() < 2 {
        return;
    }
    let rest = stack.rest();
    let row = Rect {
        y: rest.bottom() - 1,
        height: 1,
        ..rest
    };
    let mut parts: Vec<&str> = text.split(" · ").collect();
    while !parts.is_empty() && flow::width(&parts.join(" · ")) > usize::from(row.width) {
        parts.pop();
    }
    if parts.is_empty() {
        return;
    }
    frame.render_widget(
        Paragraph::new(Span::styled(parts.join(" · "), style(Role::Muted))).centered(),
        row,
    );
}

/// A title row: `text` bold in the middle.
pub(crate) fn title(frame: &mut Frame<'_>, row: Rect, text: &str) {
    frame.render_widget(
        Paragraph::new(Span::styled(
            flow::truncated(text, usize::from(row.width)),
            style(Role::Title),
        ))
        .centered(),
        row,
    );
}
