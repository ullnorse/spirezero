//! The top bar every run screen shares, drawn from [`TopBar`]: HP, gold, the floor and the menu
//! `Tab ≡` (the key that opens it named, so it can be learnt), then a rule, then the potions on
//! a row of their own: always in a fight (`No potions` when the belt is empty), elsewhere while
//! there are any. Each potion prints the key that selects it; the selected one asks in place on
//! the row. Nothing else is always on screen.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use sts2_view::model::{PotionSlot, TopBar};

use crate::app::Overlay;
use crate::hit::{Hits, Target, UiAction};
use crate::ui::column::Stack;
use crate::ui::flow::{self, Flow};
use crate::ui::theme::{Role, style};

/// Draws the bar, its rule and the potions from the top of `stack`.
pub fn draw(frame: &mut Frame<'_>, stack: &mut Stack, top: &TopBar, hits: &mut Hits) {
    draw_bar(frame, stack.take(1), top, hits);
    let rule = stack.take(1);
    frame.render_widget(
        Paragraph::new(Span::styled(
            "─".repeat(usize::from(rule.width)),
            style(Role::Muted),
        )),
        rule,
    );
    if top.fight || top.potions.iter().any(|p| !p.empty) {
        draw_potions(frame, stack.take(1), top, hits);
    }
}

/// The menu button's text at the bar's right end: the key that opens it, then `≡`.
const MENU_BUTTON: &str = "Tab ≡ ";

/// `♥ 52/66   ◉ 99   Floor 2` and `Tab ≡` at the right end.
fn draw_bar(frame: &mut Frame<'_>, area: Rect, top: &TopBar, hits: &mut Hits) {
    if area.is_empty() {
        return;
    }
    let parts = [
        Span::styled(
            format!("♥ {}/{}", top.hp, top.max_hp),
            style(Role::Hp).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("◉ {}", top.gold), style(Role::Energy)),
        Span::raw(format!("Floor {}", top.floor)),
    ];
    let menu = Target::Button(UiAction::Toggle(Overlay::Menu));
    let room = usize::from(area.width).saturating_sub(MENU_BUTTON.len());
    let text: usize = parts.iter().map(|p| flow::width(&p.content)).sum();
    let gap = (1..=3).rev().find(|g| text + g * 2 <= room).unwrap_or(1);
    let mut spans = Vec::new();
    for (i, part) in parts.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" ".repeat(gap)));
        }
        spans.push(part);
    }
    frame.render_widget(Paragraph::new(Line::from(flow::fit(&spans, room))), area);
    // The key that opens the menu, dim, then the glyph; the button covers both.
    let width = flow::coord(MENU_BUTTON.len());
    let button = Rect {
        x: area.right().saturating_sub(width),
        width: area.width.min(width),
        ..area
    };
    hits.push(button, menu);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Tab", style(Role::Muted)),
            Span::styled(" ≡ ", Style::new().add_modifier(Modifier::BOLD)),
        ])),
        button,
    );
}

/// A potion's name in at most `letters` letters, `Potion` left out: `Fire`, `Blo`.
fn potion_label(name: &str, letters: usize) -> String {
    let name = name
        .strip_prefix("Potion of ")
        .or_else(|| name.strip_suffix(" Potion"))
        .unwrap_or(name);
    flow::truncated(name, letters)
}

/// The potions: `z ◈ Fire  c ◈ Block`, each with the key that selects it (a press does too),
/// the usable ones bold, the one the focus rests on highlighted, the one waiting for its target
/// lit; their names cut until they fit. The selected potion's question takes the row: `c ◈
/// Block Potion?  Enter use · t throw away · Esc` (the last hints left out where the row is
/// narrow). An empty belt says `No potions`.
fn draw_potions(frame: &mut Frame<'_>, area: Rect, top: &TopBar, hits: &mut Hits) {
    if area.is_empty() {
        return;
    }
    let slots: Vec<(usize, &PotionSlot)> = top
        .potions
        .iter()
        .enumerate()
        .filter(|(_, p)| !p.empty)
        .collect();
    if let Some(&(slot, potion)) = slots.iter().find(|(_, p)| p.asks) {
        draw_question(frame, area, slot, potion, hits);
        return;
    }
    if slots.is_empty() {
        let empty = "No potions";
        frame.render_widget(
            Paragraph::new(Span::styled(empty, style(Role::Muted))),
            area,
        );
        draw_relics(frame, area, flow::width(empty), top, hits);
        return;
    }
    let width = usize::from(area.width);
    let letters = (3..=16)
        .rev()
        .find(|&n| {
            slots
                .iter()
                .map(|(_, p)| flow::width(&potion_label(&p.name, n)) + 7)
                .sum::<usize>()
                <= width + 2
        })
        .unwrap_or(3);
    let mut f = Flow::new(area.width);
    for (n, &(slot, potion)) in slots.iter().enumerate() {
        let target = Target::Potion(slot);
        let base = if potion.selected {
            style(Role::Selected)
        } else if potion.usable {
            Style::new().add_modifier(Modifier::BOLD)
        } else {
            style(Role::Muted)
        };
        let base = flow::focus_style(hits, target, base);
        f.token(
            Span::styled(
                potion.key.unwrap_or('?').to_string(),
                flow::focus_style(hits, target, style(Role::Key)),
            ),
            if n == 0 { 0 } else { 3 },
            Some(target),
        );
        f.token(
            Span::styled(format!("◈ {}", potion_label(&potion.name, letters)), base),
            1,
            Some(target),
        );
    }
    let used = f.used();
    let mut lines = f.finish(area, hits);
    lines.truncate(1);
    frame.render_widget(Paragraph::new(lines), area);
    draw_relics(frame, area, used, top, hits);
}

/// The fewest blank cells between the potions and the relics.
const RELIC_GAP: usize = 4;

/// The relics at the right end of the potions' row, after the `used` cells the potions take:
/// `◆ Burning Blood  ◆ Pen Nib 3` (each with its counter, hovering one for its text), or
/// `◆ 3 relics` where their names do not fit (a click lists them), or nothing where that does
/// not fit either.
fn draw_relics(frame: &mut Frame<'_>, area: Rect, used: usize, top: &TopBar, hits: &mut Hits) {
    if top.relics.is_empty() {
        return;
    }
    let room = usize::from(area.width).saturating_sub(used + RELIC_GAP);
    let chips: Vec<String> = top
        .relics
        .iter()
        .map(|r| {
            r.counter
                .map_or_else(|| format!("◆ {}", r.name), |n| format!("◆ {} {n}", r.name))
        })
        .collect();
    let named = chips.iter().map(|c| flow::width(c)).sum::<usize>() + 2 * (chips.len() - 1);
    let look = style(Role::Title);
    if named <= room {
        let mut x = area.right() - flow::coord(named);
        for (i, chip) in chips.iter().enumerate() {
            let rect = Rect {
                x,
                width: flow::coord(flow::width(chip)),
                ..area
            };
            hits.push(rect, Target::Relic(i));
            frame.render_widget(Paragraph::new(Span::styled(chip.clone(), look)), rect);
            x += rect.width + 2;
        }
        return;
    }
    let n = top.relics.len();
    let short = format!("◆ {n} relic{}", if n == 1 { "" } else { "s" });
    if flow::width(&short) <= room {
        let width = flow::coord(flow::width(&short));
        let rect = Rect {
            x: area.right() - width,
            width,
            ..area
        };
        // A click lists them, as `v` does.
        hits.push(rect, Target::Button(UiAction::Toggle(Overlay::Relics)));
        frame.render_widget(Paragraph::new(Span::styled(short, look)), rect);
    }
}

/// The selected potion's question, in place of the belt: `Enter use · t throw away · Esc`, the
/// last hints left out where the row is narrow.
fn draw_question(
    frame: &mut Frame<'_>,
    area: Rect,
    slot: usize,
    potion: &PotionSlot,
    hits: &mut Hits,
) {
    let question = format!("{} ◈ {}?  ", potion.key.unwrap_or('?'), potion.name);
    let mut tail = vec!["Enter use", "t throw away", "Esc"];
    let line = |tail: &[&str]| format!("{question}{}", tail.join(" · "));
    while !tail.is_empty() && flow::width(&line(&tail)) > usize::from(area.width) {
        tail.pop();
    }
    let text = flow::truncated(&line(&tail), usize::from(area.width));
    hits.push(area, Target::Potion(slot));
    frame.render_widget(
        Paragraph::new(Span::styled(text, flow::highlighted(style(Role::Key)))),
        area,
    );
}
