//! The act map: a tight grid of rooms with their paths, floors numbered down the left, the boss
//! in a box on top, the floors passed dimmed, `●` where you are and the keys on the nodes you
//! can travel to (the focused one on the highlight band, on the grid and in its row); under it
//! each choice as a button.

use std::collections::{HashMap, HashSet};

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use sts2_core::run::MapCoord;
use sts2_view::model::{Block, MapGrid, MapKind, Screen};

use crate::app::App;
use crate::hit::{Hits, Pane, Target, UiAction};
use crate::text;
use crate::ui::button::{self, Button};
use crate::ui::column::{self, Stack};
use crate::ui::flow;
use crate::ui::glyph;
use crate::ui::scale;
use crate::ui::theme::{self, Role, style};

/// The map's columns.
const COLUMNS: usize = 7;

/// The floor numbers' column.
const LABEL: usize = 5;
/// Cells per grid column. The grid stays this tight at every width: rooms close together read
/// as one map, and every path is one short stroke between neighbours.
const CELL: usize = 5;

/// How the grid is laid out: one tight layout everywhere (the map screen and the map sheet).
#[derive(Clone, Copy)]
pub struct Layout {
    cell: usize,
}

impl Layout {
    /// The map's one layout.
    pub const COMPACT: Self = Self { cell: CELL };

    /// The grid's width.
    const fn width(self) -> usize {
        LABEL + COLUMNS * self.cell + 1
    }

    /// Where a node's glyph sits.
    const fn x(self, col: usize) -> usize {
        LABEL + col * self.cell + self.cell / 2 + 1
    }
}

/// The symbol a room's letter is drawn with (the letter itself in ASCII), the same in the
/// grid, the rows and the legend. Elites and rest sites are emoji (`💀`, `🔥`), two cells wide,
/// so the rooms that change a run's plan stand out at a glance.
#[must_use]
pub fn symbol(letter: char) -> char {
    if glyph::is_ascii() {
        return letter;
    }
    match letter {
        'M' => '⚔',
        'E' => '💀',
        'T' => '◆',
        'R' => '🔥',
        'B' => '♛',
        'A' => '✦',
        other => other,
    }
}

/// Draws the map screen in `area`: the boss and what lies ahead over the grid, the grid in
/// the middle of the column, scrolled to keep where you are in view (the wheel moves it), then
/// the finished event's last page (or the room's receipt) if there is one, then the nodes you
/// can travel to as buttons, and a legend under them when the height allows.
#[allow(
    clippy::too_many_lines,
    reason = "one screen's layout, read top to bottom"
)]
pub fn draw(frame: &mut Frame<'_>, area: Rect, app: &App, hits: &mut Hits) {
    let Some(model) = app.room_screen() else {
        return;
    };
    let Some(map) = model.blocks.iter().find_map(|b| match b {
        Block::Map(grid) => Some(grid),
        _ => None,
    }) else {
        return;
    };
    let buttons = choices(&model, map);
    let width = usize::from(area.width);
    let notes: Vec<Line<'static>> = model
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::Text(lines) => Some(lines),
            _ => None,
        })
        .flatten()
        .map(|rich| Line::from(text::spans(rich)))
        .collect();
    let mut finished = flow::wrap_all(&notes, width);
    finished.truncate(4);
    let list_height = button::list_height(&buttons, area.width, true);
    let header = header(map, width);
    let layout = Layout::COMPACT;
    let legend = legend(&map.kinds, width);
    // The header and the legend go only where the grid keeps nine rows; the legend first.
    let grid_rows = |extra: u16| {
        area.height
            .saturating_sub(list_height + flow::coord(finished.len()) + 2 + extra)
    };
    let (show_legend, show_header) = match () {
        () if grid_rows(2) >= 9 => (true, true),
        () if grid_rows(1) >= 9 => (true, false),
        () => (false, false),
    };
    // A tall screen keeps two rows under the legend for the key hints.
    let hint_rows = 2 * u16::from(scale::get().tall);
    let below = list_height
        + flow::coord(finished.len())
        + u16::from(!finished.is_empty())
        + u16::from(show_legend)
        + hint_rows;
    // A blank row over the grid (the header's) and one under it.
    let grid = column::middle(
        Rect {
            y: area.y + 1 + u16::from(show_header),
            height: area
                .height
                .saturating_sub(below + 2 + u16::from(show_header)),
            ..area
        },
        flow::coord(layout.width()),
    );
    if show_header {
        frame.render_widget(
            Paragraph::new(Line::from(header)),
            Rect {
                y: area.y + 1,
                height: 1,
                ..area
            },
        );
    }
    let (lines, rows) = grid_lines(map, hits, layout);
    let height = usize::from(grid.height);
    let current = map.you.map_or(0, |(_, y)| usize::from(y));
    // Keep the current row near the bottom and a few rows ahead in view; the wheel moves it.
    let current_line = rows.get(&current).copied().unwrap_or(0);
    let follow = (current_line + 4).min(lines.len()).saturating_sub(height);
    let max_top = lines.len().saturating_sub(height);
    let (follow, max_top) = (
        isize::try_from(follow).unwrap_or(0),
        isize::try_from(max_top).unwrap_or(0),
    );
    let top = (follow + app.map_scroll()).clamp(0, max_top);
    hits.limit(Pane::Map, -follow, max_top - follow);
    hits.push(grid, Target::Pane(Pane::Map));
    let top = usize::try_from(top).unwrap_or(0);
    register_nodes(hits, map, grid, top, &rows, layout);
    frame.render_widget(Paragraph::new(lines).scroll((flow::coord(top), 0)), grid);
    let mut stack = Stack::new(Rect {
        y: grid.bottom() + 1,
        height: area.bottom().saturating_sub(grid.bottom() + 1),
        ..area
    });
    if !finished.is_empty() {
        let rows = stack.take(flow::coord(finished.len()));
        frame.render_widget(Paragraph::new(finished), rows);
        stack.skip(1);
    }
    let list = stack.take(list_height);
    button::draw_list(frame, list, &buttons, true, 0, hits);
    // An error or the bot's advice takes the legend's row (or the header's, with no legend).
    let note = crate::ui::notice(app);
    if show_legend {
        let row = stack.take(1);
        match note {
            Some(note) => frame.render_widget(Paragraph::new(Line::from(note)), row),
            None => frame.render_widget(Paragraph::new(Line::from(legend)), row),
        }
    } else if let (Some(note), true) = (note, show_header) {
        let row = Rect {
            y: area.y + 1,
            height: 1,
            ..area
        };
        frame.render_widget(Clear, row);
        frame.render_widget(Paragraph::new(Line::from(note)), row);
    }
    if hint_rows > 0 {
        let pick = button::pick_hint(buttons.iter().map(|b| b.key.as_str()));
        let mut hint = vec!["↑↓ move", "Enter travel"];
        if !pick.is_empty() {
            hint.push(&pick);
        }
        hint.extend(["m map", "? help", "Tab menu"]);
        crate::ui::hints(frame, &stack, &hint.join(" · "));
    }
}

/// The line over the grid: the act's boss, the easy hallway fights left and the rooms still
/// reachable by kind (`Act 1 · Boss Vantom · 2 easy fights left · ahead: ⚔ 9 💀 2 ? 4 $ 1 ◆ 1
/// 🔥 3`), the word `ahead` left out when the column is too narrow for it.
fn header(grid: &MapGrid, width: usize) -> Vec<Span<'static>> {
    let boss = grid
        .boss
        .strip_suffix(" Boss")
        .unwrap_or(&grid.boss)
        .to_owned();
    let build = |ahead_word: bool, long: bool, terse: u8| {
        let mut spans = Vec::new();
        if grid.act > 0 {
            spans.push(Span::styled(
                format!("Act {} · ", grid.act),
                Style::new().add_modifier(Modifier::BOLD),
            ));
        }
        spans.extend([
            Span::styled(if terse > 0 { "" } else { "Boss " }, style(Role::Muted)),
            Span::styled(
                if terse > 1 {
                    String::new()
                } else {
                    boss.clone()
                },
                Style::new().add_modifier(Modifier::BOLD),
            ),
        ]);
        // The act's easy hallway fights: how many are still to come (shorter when narrow).
        let easy = match (grid.easy_fights_left, long) {
            (0, true) => " · easy fights done".to_owned(),
            (0, false) => " · easy done".to_owned(),
            (1, true) => " · 1 easy fight left".to_owned(),
            (n, true) => format!(" · {n} easy fights left"),
            (n, false) => format!(" · {n} easy left"),
        };
        let easy = if terse > 0 {
            easy.replace(" left", "").replace(" fights", "")
        } else {
            easy
        };
        // With no boss name the easy-fights count follows the act directly.
        let easy = if terse > 1 {
            easy.trim_start_matches(" · ").to_owned()
        } else {
            easy
        };
        spans.push(Span::styled(easy, style(Role::Muted)));
        spans.push(Span::styled(
            if ahead_word { " · ahead:" } else { " ·" },
            style(Role::Muted),
        ));
        for kind in grid.kinds.iter().filter(|k| k.ahead > 0) {
            spans.push(Span::styled(
                format!(" {}", symbol(kind.glyph)),
                room_style(kind.glyph),
            ));
            spans.push(Span::styled(format!(" {}", kind.ahead), style(Role::Muted)));
        }
        spans
    };
    let fits =
        |spans: &Vec<Span<'static>>| flow::width(&Line::from(spans.clone()).to_string()) <= width;
    // The whole header first, then without "ahead:", then in shorter words, so that the counts of
    // the rooms ahead are never the part that is cut.
    [
        build(true, true, 0),
        build(false, true, 0),
        build(false, false, 0),
        build(false, false, 1),
        build(false, false, 2),
    ]
    .into_iter()
    .find(fits)
    .unwrap_or_else(|| flow::fit(&build(false, false, 0), width))
}

/// The legend under the choices: `⚔ monster  ☠ elite  ? unknown  $ shop  ◆ treasure  ♨ rest  ● you`
/// (treasure left out when the column is too narrow for it).
fn legend(kinds: &[MapKind], width: usize) -> Vec<Span<'static>> {
    let fits =
        |spans: &Vec<Span<'static>>| flow::width(&Line::from(spans.clone()).to_string()) <= width;
    let full = legend_spans(kinds.iter(), true, false);
    if fits(&full) {
        return full;
    }
    let no_treasure = legend_spans(kinds.iter().filter(|k| k.room != "Treasure"), true, false);
    if fits(&no_treasure) {
        return no_treasure;
    }
    flow::fit(
        &legend_spans(kinds.iter().filter(|k| k.room != "Treasure"), false, true),
        width,
    )
}

/// The legend's spans; `tight` leaves one space between the entries instead of two.
fn legend_spans<'a>(
    kinds: impl Iterator<Item = &'a MapKind>,
    you: bool,
    tight: bool,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for kind in kinds {
        spans.push(Span::styled(
            format!(" {}", symbol(kind.glyph)),
            room_style(kind.glyph),
        ));
        let word = if tight {
            format!(" {}", kind.word)
        } else {
            format!(" {} ", kind.word)
        };
        spans.push(Span::styled(word, style(Role::Body)));
    }
    if you {
        spans.push(Span::styled(" ●", style(Role::Selected)));
        spans.push(Span::styled(" you", style(Role::Body)));
    }
    spans
}

/// The nodes you can travel to as buttons, each with what one path from it passes before the
/// boss, fewest to most: `1  ⚔  Monster   🔥1–3 💀0–2 $1 ?2–4`.
fn choices(screen: &Screen, grid: &MapGrid) -> Vec<Button> {
    let mut buttons = Vec::new();
    for block in &screen.blocks {
        let Block::List { rows, .. } = block else {
            continue;
        };
        for row in rows {
            let key = row.key.map_or_else(String::new, |k| match k {
                sts2_view::model::Key::Digit(d) => d.to_string(),
                sts2_view::model::Key::Char(c) => c.to_string(),
                _ => String::new(),
            });
            buttons.push(
                Button::new(
                    key,
                    room_label(text::spans(&row.label)),
                    Some(Target::Button(UiAction::Pick(row.id.0 as usize))),
                )
                .right(
                    grid.cells
                        .iter()
                        .find(|c| c.button == Some(row.id))
                        .map(|c| ahead_spans(grid, (c.x, c.y)))
                        .or_else(|| row.right.as_ref().map(|r| symbols(text::spans(r), 5)))
                        .unwrap_or_default(),
                )
                // The mark costs a column the narrow rows cannot spare; `i` works there.
                .more(scale::big()),
            );
        }
    }
    buttons
}

/// The rooms the paths from `from` reach (the boss's included), `from` among them.
fn route(grid: &MapGrid, from: (u8, u8)) -> HashSet<(u8, u8)> {
    let mut seen = HashSet::from([from]);
    let mut stack = vec![from];
    while let Some(at) = stack.pop() {
        for edge in grid.edges.iter().filter(|e| e.from == at) {
            if seen.insert(edge.to) {
                stack.push(edge.to);
            }
        }
    }
    seen
}

/// The rooms that change a run's plan, in the order a next-room row counts them.
const PLANNED: [char; 5] = ['R', 'E', '$', 'T', '?'];

/// The fewest and the most rooms with `letter` that one path from `at` to the boss passes,
/// `at` itself counted; `memo` keeps each room's answer.
fn path_range(
    grid: &MapGrid,
    letter: char,
    at: (u8, u8),
    memo: &mut HashMap<(u8, u8), (usize, usize)>,
) -> (usize, usize) {
    if let Some(&known) = memo.get(&at) {
        return known;
    }
    let own = usize::from(
        grid.cells
            .iter()
            .any(|c| (c.x, c.y) == at && c.glyph == letter),
    );
    let mut next: Option<(usize, usize)> = None;
    for edge in grid.edges.iter().filter(|e| e.from == at) {
        let (lo, hi) = path_range(grid, letter, edge.to, memo);
        next = Some(next.map_or((lo, hi), |(a, b)| (a.min(lo), b.max(hi))));
    }
    let (lo, hi) = next.unwrap_or((0, 0));
    let range = (own + lo, own + hi);
    memo.insert(at, range);
    range
}

/// What one path from the room at `from` passes before the boss, by kind, as the fewest and
/// the most any single path gives: `🔥1–3 💀0–2 $1 ?2–4` (rests, elites, shops, treasure,
/// unknown rooms, the room itself counted; one number where every path gives the same), each in
/// its room's look; a kind no path passes is left out.
fn ahead_spans(grid: &MapGrid, from: (u8, u8)) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for letter in PLANNED {
        let (lo, hi) = path_range(grid, letter, from, &mut HashMap::new());
        if hi == 0 {
            continue;
        }
        if !spans.is_empty() {
            spans.push(Span::raw(" "));
        }
        let count = if lo == hi {
            hi.to_string()
        } else {
            format!("{lo}–{hi}")
        };
        spans.push(Span::styled(
            format!("{}{count}", symbol(letter)),
            room_style(letter),
        ));
    }
    spans
}

/// A next-room row's label: its room letter drawn as the room's symbol in the room's look.
fn room_label(mut spans: Vec<Span<'static>>) -> Vec<Span<'static>> {
    if let Some(first) = spans.first_mut() {
        let letter = first.content.chars().next().unwrap_or(' ');
        first.style = room_style(letter);
    }
    symbols(spans, 0)
}

/// `spans` with the first span's characters from `from` on drawn as room symbols (the glyph
/// of a row's label, the letters after `next ` on its right).
fn symbols(mut spans: Vec<Span<'static>>, from: usize) -> Vec<Span<'static>> {
    if let Some(first) = spans.first_mut() {
        first.content = first
            .content
            .chars()
            .enumerate()
            .map(|(i, c)| if i >= from { symbol(c) } else { c })
            .collect::<String>()
            .into();
    }
    spans
}

/// The full map as lines, for the enlarged view, with the line of each row.
pub fn full_lines(grid: &MapGrid, hits: &Hits) -> (Vec<Line<'static>>, HashMap<usize, usize>) {
    grid_lines(grid, hits, Layout::COMPACT)
}

/// Registers every node of the grid drawn in `area` scrolled `top` lines down; `rows` maps each
/// map row onto its line.
pub fn register_nodes(
    hits: &mut Hits,
    grid: &MapGrid,
    area: Rect,
    top: usize,
    rows: &HashMap<usize, usize>,
    layout: Layout,
) {
    for cell in &grid.cells {
        let (col, row) = (usize::from(cell.x), usize::from(cell.y));
        let Some(y) = rows
            .get(&row)
            .and_then(|line| line.checked_sub(top))
            .filter(|y| *y < usize::from(area.height))
        else {
            continue;
        };
        let x = layout.x(col) - 3;
        if x >= usize::from(area.width) {
            continue;
        }
        let rect = Rect {
            x: area.x + flow::coord(x),
            y: area.y + flow::coord(y),
            width: 4.min(area.width - flow::coord(x)),
            height: 1,
        };
        hits.push(rect, Target::MapNode(MapCoord { col, row }));
    }
}

/// One drawn row of the grid: a character and its style per cell.
///
/// A two-cell character (an emoji) covers the cell after it, which is then left out of the line
/// and ignores what is put there later.
struct Canvas(Vec<(char, Style)>);

/// The cell a two-cell character covers.
const COVERED: char = '\0';

impl Canvas {
    fn new(width: usize) -> Self {
        Self(vec![(' ', Style::new()); width])
    }

    fn put(&mut self, x: usize, ch: char, style: Style) {
        match self.0.get_mut(x) {
            Some(slot) if slot.0 != COVERED => *slot = (ch, style),
            _ => return,
        }
        if flow::width(&ch.to_string()) > 1
            && let Some(next) = self.0.get_mut(x + 1)
        {
            *next = (COVERED, style);
        }
    }

    fn text(&mut self, x: usize, text: &str, style: Style) {
        for (i, ch) in text.chars().enumerate() {
            self.put(x + i, ch, style);
        }
    }

    fn line(self) -> Line<'static> {
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut run = String::new();
        let mut current = None;
        for (ch, style) in self.0 {
            if current.is_some_and(|c| c != style) {
                spans.push(Span::styled(
                    std::mem::take(&mut run),
                    current.unwrap_or_default(),
                ));
            }
            current = Some(style);
            if ch != COVERED {
                run.push(ch);
            }
        }
        if let Some(style) = current {
            spans.push(Span::styled(run, style));
        }
        Line::from(spans)
    }
}

/// The grid from the boss down to the start, rows of paths between floors, and the line each
/// row is on.
fn grid_lines(
    grid: &MapGrid,
    hits: &Hits,
    layout: Layout,
) -> (Vec<Line<'static>>, HashMap<usize, usize>) {
    let you = grid.you.map(|(x, y)| (usize::from(x), usize::from(y)));
    let current_row = you.map_or(0, |(_, y)| y);
    // While a next-room row is focused, its paths are lit and what they miss is dimmed.
    let focused = grid.cells.iter().find(|c| {
        c.button
            .is_some_and(|id| hits.focused(Target::Button(UiAction::Pick(id.0 as usize))))
    });
    let route = focused.map(|c| route(grid, (c.x, c.y)));
    let width = layout.width() + 2;
    let mut lines = Vec::new();
    let mut rows = HashMap::new();
    let (boss_col, boss_row) = (usize::from(grid.boss_at.0), usize::from(grid.boss_at.1));
    let boss_x = layout.x(boss_col);
    let boss_style = style(Role::Attack);
    let mut top = Canvas::new(width);
    top.text(boss_x - 4, "┌──────┐", boss_style);
    lines.push(top.line());
    rows.insert(boss_row, lines.len());
    let mut mid = Canvas::new(width);
    mid.text(0, &format!("{:>3}", boss_row + 1), style(Role::Muted));
    mid.text(boss_x - 4, "│", boss_style);
    mid.text(
        boss_x - 3,
        " BOSS ",
        boss_style.add_modifier(Modifier::BOLD),
    );
    mid.text(boss_x + 3, "│", boss_style);
    let mut mid_line = mid.line();
    mid_line
        .spans
        .push(Span::styled(format!("  {}", grid.boss), style(Role::Muted)));
    lines.push(mid_line);
    let mut bottom = Canvas::new(width);
    bottom.text(boss_x - 4, "└──────┘", boss_style);
    lines.push(bottom.line());
    for row in (0..boss_row).rev() {
        lines.push(connectors(grid, row + 1, layout, you, route.as_ref()));
        rows.insert(row, lines.len());
        let passed = you.is_some() && row < current_row;
        let mut canvas = Canvas::new(width);
        let label = format!("{:>3}", row + 1);
        let label_style = if you.is_some_and(|(_, y)| y == row) {
            style(Role::Selected)
        } else {
            style(Role::Muted)
        };
        canvas.text(0, &label, label_style);
        for cell in grid.cells.iter().filter(|c| usize::from(c.y) == row) {
            let x = layout.x(usize::from(cell.x));
            let g = symbol(cell.glyph);
            if you == Some((usize::from(cell.x), row)) {
                canvas.text(x - 3, "   ", style(Role::Selected));
                canvas.put(x, symbol('●'), style(Role::Selected));
                canvas.put(x + 1, ' ', style(Role::Selected));
            } else if let Some(id) = cell.button {
                // The key in the keys' colour, the room in its own; both on the band while the
                // focus rests on it.
                let lit = hits.focused(Target::Button(UiAction::Pick(id.0 as usize)));
                let on = |s: Style| if lit { flow::highlighted(s) } else { s };
                canvas.text(x - 3, &format!("[{}]", id.0), on(style(Role::Key)));
                canvas.put(x, g, on(room_style(cell.glyph)));
                canvas.put(x + 1, ' ', on(Style::new()));
            } else {
                let missed = route
                    .as_ref()
                    .is_some_and(|r| !r.contains(&(cell.x, cell.y)));
                let base = if passed || cell.visited {
                    style(Role::Muted)
                } else if cell.ahead && !missed {
                    room_style(cell.glyph)
                } else {
                    style(Role::Muted).add_modifier(Modifier::DIM)
                };
                canvas.put(x, g, base);
            }
        }
        lines.push(canvas.line());
    }
    (lines, rows)
}

/// The paths between `row` (above) and the row below it: one row, a path leaning from its node
/// toward the node it leads to, halfway between them. Paths from where you stand are bright,
/// those still ahead plain, the rest dim; with a focused next room's `route`, its paths (and
/// the step into it) are bright and every other path dim.
fn connectors(
    grid: &MapGrid,
    row: usize,
    layout: Layout,
    you: Option<(usize, usize)>,
    route: Option<&HashSet<(u8, u8)>>,
) -> Line<'static> {
    let mut canvas = Canvas::new(layout.width() + 2);
    for edge in grid
        .edges
        .iter()
        .filter(|e| usize::from(e.from.1) + 1 == row)
    {
        let from = grid.cells.iter().find(|c| (c.x, c.y) == edge.from);
        let (col, to_col) = (usize::from(edge.from.0), usize::from(edge.to.0));
        let next = you.map_or(edge.from.1 == 0, |(x, y)| {
            (x, y) == (col, usize::from(edge.from.1))
        });
        let on_route = route.map(|r| r.contains(&edge.to) && (next || r.contains(&edge.from)));
        let ahead = from.is_some_and(|c| c.ahead && !c.visited)
            && you.is_none_or(|(_, y)| usize::from(edge.from.1) >= y);
        let bright = style(Role::Key).add_modifier(Modifier::BOLD);
        let dim = style(Role::Muted).add_modifier(Modifier::DIM);
        let edge_style = match on_route {
            Some(true) => bright,
            None if next => bright,
            None if ahead => Style::new(),
            Some(false) | None => dim,
        };
        let (x0, x1) = (layout.x(col), layout.x(to_col));
        // Halfway, rounded toward the lower node, so leaning either way sits alike.
        let x = if x1 >= x0 {
            x0 + (x1 - x0) / 2
        } else {
            x0 - (x0 - x1) / 2
        };
        let ch = match to_col.cmp(&col) {
            std::cmp::Ordering::Less => '╲',
            std::cmp::Ordering::Equal => '│',
            std::cmp::Ordering::Greater => '╱',
        };
        let old = canvas.0.get(x).map_or(' ', |c| c.0);
        let ch = if old == ' ' || old == ch { ch } else { '╳' };
        canvas.put(x, ch, edge_style);
    }
    canvas.line()
}

/// A room's look by its letter, the same on the grid, the rows, the header and the legend: plain
/// fights, the most of the map, quiet in grey so the rest stand out; elites and the boss red,
/// rest sites amber like a fire, shops and treasure gold, unknown rooms purple, each bold.
const fn room_style(letter: char) -> Style {
    let look = match letter {
        'M' => return style(Role::Body),
        'E' | 'B' => Style::new().fg(theme::ELITE),
        'R' => Style::new().fg(theme::FIRE),
        '$' | 'T' => style(Role::Energy),
        '?' => style(Role::Debuff),
        _ => Style::new(),
    };
    look.add_modifier(Modifier::BOLD)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sts2_view::model::{MapCell, MapEdge};

    /// A room's place on the grid.
    type At = (u8, u8);

    /// `cells` as `(x, y, letter)` with `edges` between them, the boss at (0, 9).
    fn grid(cells: &[(u8, u8, char)], edges: &[(At, At)]) -> MapGrid {
        MapGrid {
            cells: cells
                .iter()
                .map(|&(x, y, glyph)| MapCell {
                    x,
                    y,
                    glyph,
                    ..MapCell::default()
                })
                .collect(),
            edges: edges
                .iter()
                .map(|&(from, to)| MapEdge { from, to })
                .collect(),
            boss_at: (0, 9),
            ..MapGrid::default()
        }
    }

    #[test]
    fn a_rooms_counts_are_what_one_path_passes() {
        // Start (0,0) splits: left through two elites, right through a rest; both reach the
        // boss. Every elite on the map is three, but no path passes more than two.
        let map = grid(
            &[
                (0, 0, 'M'),
                (0, 1, 'E'),
                (0, 2, 'E'),
                (1, 1, 'R'),
                (1, 2, 'E'),
            ],
            &[
                ((0, 0), (0, 1)),
                ((0, 1), (0, 2)),
                ((0, 0), (1, 1)),
                ((1, 1), (1, 2)),
                ((0, 2), (0, 9)),
                ((1, 2), (0, 9)),
            ],
        );
        assert_eq!(path_range(&map, 'E', (0, 0), &mut HashMap::new()), (1, 2));
        assert_eq!(path_range(&map, 'R', (0, 0), &mut HashMap::new()), (0, 1));
        assert_eq!(path_range(&map, 'E', (1, 1), &mut HashMap::new()), (1, 1));
        let words: String = ahead_spans(&map, (0, 0))
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(words, "🔥0–1 💀1–2");
    }
}
