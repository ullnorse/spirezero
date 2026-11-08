//! The enemies, one under another, or two to a row when rows are short: each one's number and
//! name, its intent, its HP bar with its Block (what the selected card would do to it drawn
//! into them; two rows thick on a big screen), and its powers; shorter still, each on one
//! row.
//!
//! Nothing here moves as the focus moves: the name line keeps a cell for the `▸` marker, and
//! the HP line is placed by its bar, numbers and Block alone, so a preview lights the number
//! in its slot and the cells the play takes, and its note goes after, where there is room.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use sts2_core::combat::CreatureId;
use sts2_view::model::EnemyRow;

use super::{
    BarRow, Big, Plan, Preview, View, bar_spans, block_span, centred, has_powers, hp_numbers_width,
    hp_spans, numbers_span, power_tokens,
};
use crate::hit::{Hits, Target};
use crate::text;
use crate::ui::column::Stack;
use crate::ui::flow::{self, Flow};
use crate::ui::scale;
use crate::ui::theme::{Role, style};

/// The widest an HP bar gets; half as much again on a wide column.
const MAX_BAR: usize = 16;

/// `name` cut to `width` cells: leading words dropped first (`Fuzzy Wurm Crawler` → `Wurm
/// Crawler`), then the end cut with `…`.
#[must_use]
pub fn short_name(name: &str, width: usize) -> String {
    let mut words: &str = name;
    while flow::width(words) > width {
        match words.split_once(' ') {
            Some((_, rest)) if !rest.starts_with('(') => words = rest,
            _ => break,
        }
    }
    flow::truncated(words, width)
}

/// The intent row: each intent's glyph and words, `⚔ 6×3  ▼ Weak 2`; on a big screen the
/// damage spelled out.
fn intent_line(enemy: &EnemyRow) -> Line<'static> {
    let intent = if scale::big() {
        enemy.intent_long.as_ref()
    } else {
        enemy.intent.as_ref()
    };
    Line::from(intent.map(text::spans).unwrap_or_default())
}

/// The name row: `1 Leaf Slime (M)`, the number bright; in the keys' colour while the enemy can
/// be picked as the target, marked `▸` and reversed while the focus rests on it. The marker's
/// cell is there (blank) whether or not it is, so the name stays put.
fn name_line(enemy: &EnemyRow, slot: usize, width: usize, hits: &Hits) -> Line<'static> {
    let id = CreatureId(enemy.id);
    let key = (slot + 1).to_string();
    let name = enemy.name.clone();
    if !enemy.alive {
        let room = width.saturating_sub(2);
        return Line::from(Span::styled(
            format!("✗ {}", short_name(&name, room)),
            style(Role::Muted),
        ));
    }
    let focused = hits.focused(Target::Creature(id));
    let pick = enemy.target.is_some();
    let name = short_name(&name, width.saturating_sub(key.len() + 4));
    let base = if focused {
        style(Role::Selected)
    } else if pick {
        style(Role::Key)
    } else {
        style(Role::Title)
    };
    let marker = if focused { "▸" } else { " " };
    Line::from(vec![
        Span::styled(marker, base),
        Span::styled(key, if pick { base } else { style(Role::Key) }),
        Span::styled(" ", if pick { base } else { Style::new() }),
        Span::styled(name, base),
        Span::styled(" ›", style(Role::Muted)),
    ])
}

/// The rows `id` takes: its name alone once dead, else name, intent and HP (two rows thick
/// on a big screen), and its powers; one row in a compact plan.
fn rows(plan: Plan, big: Big, enemy: &EnemyRow) -> u16 {
    if enemy.alive && !plan.compact() {
        2 + bar_rows(big) + u16::from(plan.powers() && has_powers(&enemy.powers))
    } else {
        1
    }
}

/// The rows an HP bar takes: two when thick.
const fn bar_rows(big: Big) -> u16 {
    if big.thick { 2 } else { 1 }
}

/// The fewest columns an enemy takes when the enemies stand side by side.
const SIDE_BY_SIDE: u16 = 32;

/// How many enemies share a row `width` wide: all of them side by side across the top, as in
/// the game, where each gets [`SIDE_BY_SIDE`] columns; else one each, or two to a row in a
/// tight plan.
fn per_row(view: &View<'_>, plan: Plan, width: u16) -> usize {
    let n = view.enemies.len();
    if n > 1 && !plan.compact() && width / flow::coord(n) >= SIDE_BY_SIDE {
        n
    } else if plan.pairs() {
        2
    } else {
        1
    }
}

/// The rows of enemies `width` wide.
pub fn blocks(view: &View<'_>, plan: Plan, width: u16) -> usize {
    view.enemies.len().div_ceil(per_row(view, plan, width))
}

/// Where each enemy goes in `area` (only its width and top matter), `gap` blank rows between
/// the rows of enemies: `(enemy, slot, rect)`.
fn places<'a>(
    view: &View<'a>,
    plan: Plan,
    big: Big,
    area: Rect,
    gap: u16,
) -> Vec<(&'a EnemyRow, usize, Rect)> {
    let per_row = per_row(view, plan, area.width);
    let mut out = Vec::new();
    let mut y = area.y;
    for (row, chunk) in view.enemies.chunks(per_row).enumerate() {
        if row > 0 {
            y += gap;
        }
        let height = chunk.iter().map(|e| rows(plan, big, e)).max().unwrap_or(0);
        let width = area.width / flow::coord(chunk.len());
        for (i, &enemy) in chunk.iter().enumerate() {
            let x = area.x + width * flow::coord(i);
            // The last of a row takes what rounding left.
            let w = if i + 1 == chunk.len() {
                area.right() - x
            } else {
                width
            };
            out.push((
                enemy,
                row * per_row + i,
                Rect::new(x, y, w, rows(plan, big, enemy)),
            ));
        }
        y += height;
    }
    out
}

/// The rows the enemies take with `gap` blank rows between their rows.
pub fn height(view: &View<'_>, plan: Plan, big: Big, width: u16, gap: u16) -> u16 {
    places(view, plan, big, Rect::new(0, 0, width, 0), gap)
        .iter()
        .map(|(_, _, r)| r.bottom())
        .max()
        .unwrap_or(0)
}

/// Draws the enemies from the top of `stack`.
pub fn draw(
    frame: &mut Frame<'_>,
    stack: &mut Stack,
    view: &View<'_>,
    plan: Plan,
    big: Big,
    gap: u16,
    hits: &mut Hits,
) {
    let area = stack.take(height(view, plan, big, stack.rest().width, gap));
    for (enemy, slot, rect) in places(view, plan, big, area, gap) {
        draw_one(frame, rect.intersection(area), enemy, slot, plan, big, hits);
    }
}

/// One enemy in `rect`, each row centred.
fn draw_one(
    frame: &mut Frame<'_>,
    rect: Rect,
    enemy: &EnemyRow,
    slot: usize,
    plan: Plan,
    big: Big,
    hits: &mut Hits,
) {
    if rect.is_empty() {
        return;
    }
    hits.push(rect, Target::Creature(CreatureId(enemy.id)));
    let row = |n: u16| Rect {
        y: rect.y + n,
        height: u16::from(n < rect.height),
        ..rect
    };
    let width = usize::from(rect.width);
    if plan.compact() && enemy.alive {
        draw_compact(frame, row(0), enemy, slot, hits);
        return;
    }
    let mut name = name_line(enemy, slot, width.saturating_sub(1), hits);
    // Lit while its move plays out in the enemies' turn.
    if crate::ui::motion::acting(enemy.id) {
        name = flow::banded(name);
    }
    let at = centred(frame, row(0), &name.spans);
    super::draw_hit(frame, row(0), at, enemy.id);
    if !enemy.alive || rect.height < 3 {
        return;
    }
    let intent = intent_line(enemy);
    if !intent.spans.is_empty() {
        centred(frame, row(1), &intent.spans);
    }
    let bars = bar_rows(big);
    let first = if bars > 1 {
        BarRow::Top
    } else {
        BarRow::Single
    };
    let preview = preview_of(enemy, hits);
    draw_hp(frame, row(2), enemy, preview, first);
    if bars > 1 {
        draw_hp(frame, row(3), enemy, preview, BarRow::Bottom);
    }
    if plan.powers() && rect.height > 2 + bars {
        // A cell of margin at the right keeps the powers of neighbours apart; what does not fit
        // is counted (`+2`), the rest is in the enemy's details.
        let inner = rect.width.saturating_sub(1);
        let mut f = Flow::new(inner);
        let hidden = power_tokens(&mut f, enemy.id, &enemy.powers, rect.width < 30);
        if hidden > 0 {
            let more = format!("+{hidden}");
            let gap = usize::from(f.width_left() != f.width());
            if f.width_left() >= gap + flow::width(&more) {
                f.token(Span::styled(more, style(Role::Muted)), gap, None);
            }
        }
        let used = f.width() - f.width_left();
        let origin = Rect {
            x: rect.x + (inner - flow::coord(used)) / 2,
            width: flow::coord(used),
            ..row(2 + bars)
        };
        let mut lines = f.finish(origin, hits);
        lines.truncate(1);
        frame.render_widget(Paragraph::new(lines), origin);
    }
}

/// What the waiting (or focused) card does to `enemy`; nothing while it is aimed at another
/// enemy, since the aim says which one it hits.
fn preview_of(enemy: &EnemyRow, hits: &Hits) -> Preview {
    enemy
        .preview
        .filter(|_| !hits.aiming_elsewhere(CreatureId(enemy.id)))
        .map(Preview::of)
        .unwrap_or_default()
}

/// The widest an HP bar gets on a compact row.
const COMPACT_BAR: usize = 8;

/// An enemy on one row, `1 Leaf Slime (S)  ⚔ 3  ████░░ 8/14 ▣ 5`: the name cut to leave
/// the intent and the HP their room, the preview drawn into the HP as on the HP line.
fn draw_compact(frame: &mut Frame<'_>, row: Rect, enemy: &EnemyRow, slot: usize, hits: &Hits) {
    if row.is_empty() {
        return;
    }
    let width = usize::from(row.width);
    let preview = preview_of(enemy, hits);
    let intent = intent_line(enemy);
    let intent_width: usize = intent.spans.iter().map(|s| flow::width(&s.content)).sum();
    let block = block_span(enemy.block, preview.block_after);
    let block_width = block.as_ref().map_or(0, |b| 1 + flow::width(&b.content));
    let numbers = hp_numbers_width(enemy.max_hp);
    let bar = width
        .saturating_sub(NAME_ROOM + 2 + intent_width + 2 + numbers + block_width)
        .clamp(3, COMPACT_BAR);
    let name_room = width.saturating_sub(2 + intent_width + 2 + bar + numbers + block_width);
    let mut spans = name_line(enemy, slot, name_room, hits).spans;
    spans.push(Span::raw("  "));
    spans.extend(intent.spans);
    spans.push(Span::raw("  "));
    spans.extend(hp_spans(
        hp_of(enemy),
        bar,
        Role::Hp,
        preview.hp_after,
        BarRow::Single,
    ));
    if let Some(block) = block {
        spans.push(Span::raw(" "));
        spans.push(block);
    }
    // What the play takes, when the row has room for it whole.
    let used: usize = spans.iter().map(|s| flow::width(&s.content)).sum();
    if let Some(span) = preview
        .tail()
        .into_iter()
        .next()
        .filter(|s| used + 1 + flow::width(&s.content) <= width)
    {
        spans.push(Span::raw(" "));
        spans.push(span);
    }
    frame.render_widget(Paragraph::new(Line::from(flow::fit(&spans, width))), row);
}

/// An enemy's HP, its maximum and whether it lives, for the bar.
const fn hp_of(enemy: &EnemyRow) -> (i32, i32, bool) {
    (enemy.hp, enemy.max_hp, enemy.alive)
}

/// The fewest cells a compact row keeps for the enemy's name before the bar shrinks.
const NAME_ROOM: usize = 12;

/// The HP line in `row`: the bar, the numbers and the Block centred as they are without any
/// preview, the preview drawn into them, the HP it takes and its note after them when they fit
/// whole. The bottom row of a thick bar is the bar alone, under the first.
fn draw_hp(frame: &mut Frame<'_>, row: Rect, enemy: &EnemyRow, preview: Preview, bar_row: BarRow) {
    let second = bar_row == BarRow::Bottom;
    if row.is_empty() {
        return;
    }
    let block = block_span(enemy.block, preview.block_after);
    let numbers = hp_numbers_width(enemy.max_hp);
    let block_width = block.as_ref().map_or(0, |b| 2 + flow::width(&b.content));
    let width = usize::from(row.width);
    let widest = if scale::big() {
        MAX_BAR * 2
    } else if scale::wide() {
        MAX_BAR + MAX_BAR / 2
    } else {
        MAX_BAR
    };
    let bar = width
        .saturating_sub(numbers + block_width + 1)
        .clamp(3, widest);
    let stable = bar + numbers + block_width;
    // A hit drains first; the card's preview shows again once it rests.
    let draining = super::draining_bar(
        (enemy.id, enemy.hp, enemy.max_hp, enemy.alive),
        bar,
        Role::Hp,
        bar_row,
    );
    let mut spans = match (draining, second) {
        (Some(mut drawn), false) => {
            drawn.push(numbers_span(enemy.hp, enemy.max_hp, false));
            drawn
        }
        (Some(drawn), true) => drawn,
        (None, true) => bar_spans(hp_of(enemy), bar, Role::Hp, preview.hp_after, bar_row),
        (None, false) => hp_spans(hp_of(enemy), bar, Role::Hp, preview.hp_after, bar_row),
    };
    if let Some(block) = block.filter(|_| !second) {
        spans.push(Span::raw("  "));
        spans.push(block);
    }
    let x = row.x + (row.width.saturating_sub(flow::coord(stable))) / 2;
    let room = usize::from(row.right().saturating_sub(x));
    // What the play takes, then its note, each only when it fits whole.
    let mut used = stable;
    for span in preview.tail() {
        let w = 2 + flow::width(&span.content);
        if second || used + w > room {
            break;
        }
        used += w;
        spans.push(Span::raw("  "));
        spans.push(span);
    }
    let rect = Rect {
        x,
        width: flow::coord(room),
        ..row
    };
    frame.render_widget(Paragraph::new(Line::from(flow::fit(&spans, room))), rect);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_drop_leading_words_before_letters() {
        assert_eq!(short_name("Fuzzy Wurm Crawler", 12), "Wurm Crawler");
        assert_eq!(short_name("Fuzzy Wurm Crawler", 20), "Fuzzy Wurm Crawler");
        assert_eq!(short_name("Leaf Slime (M)", 10), "Slime (M)");
        assert_eq!(short_name("Leaf Slime (M)", 8), "Slime (…");
        assert_eq!(short_name("Crawler", 4), "Cra…");
    }
}
