//! The player: the name, the HP bar and the Block, Osty's own bar under it, the powers, and
//! the character's resources (orbs, the Sovereign Blade). What the enemies' next turn takes
//! is drawn into the bar in a tint of its own and said on the row under it (`⚔ −7 HP next
//! turn`). On a big screen the bar is two rows thick.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use sts2_core::combat::{CardId, CreatureId};
use sts2_view::model::{IncomingRow, OrbRow};

use super::{
    BarRow, Big, Plan, View, block_span, has_powers, hp_numbers_width, hp_spans, numbers_span,
    power_tokens, tinted_bar,
};
use crate::hit::{Hits, Target};
use crate::ui::column::Stack;
use crate::ui::flow::{self, Flow};
use crate::ui::scale;
use crate::ui::theme::{self, Role, style};

/// The widest the player's HP bar gets; twice that on a wide column, three times on a big one.
const MAX_BAR: usize = 16;

/// What the enemies' next turn costs you, as the engine worked it out, in one line `width`
/// wide, saying it is the next turn's: the HP lost after Block and Osty, `⚔ −7 HP next turn`
/// (`⚔ −7 HP or more next turn` when the engine cannot be sure: a hidden intent, a draw or a
/// roll in the enemy turn); `☠ −80 HP next turn  LETHAL` when it kills; `▣ All blocked next
/// turn` when the Block takes it all (`so far` when unsure); `No incoming damage`, dim, when
/// nothing comes. The intents' other effects (debuffs, status cards) are on the enemies'
/// intent rows.
fn incoming_spans(incoming: &IncomingRow, width: usize) -> Vec<Span<'static>> {
    let spans = if incoming.lethal {
        vec![
            Span::styled(
                format!("☠ −{} HP next turn", incoming.hp_loss),
                style(Role::Attack).bold(),
            ),
            Span::raw("  "),
            Span::styled(" LETHAL ", style(Role::Lethal)),
        ]
    } else if incoming.hp_loss > 0 {
        let more = if incoming.certain { "" } else { " or more" };
        vec![Span::styled(
            format!("⚔ −{} HP{more} next turn", incoming.hp_loss),
            style(Role::Attack).bold(),
        )]
    } else if incoming.total > 0 {
        let when = if incoming.certain {
            "next turn"
        } else {
            "so far"
        };
        vec![Span::styled(
            format!("▣ All blocked {when}"),
            style(Role::Defend).bold(),
        )]
    } else if incoming.certain {
        vec![Span::styled("No incoming damage", style(Role::Muted))]
    } else {
        vec![Span::styled("⚔ next turn: unknown", style(Role::Muted))]
    };
    flow::fit(&spans, width)
}

/// [`incoming_spans`], dim while it is kept from before the turn's last play (a card choice).
fn incoming_words(incoming: &IncomingRow, width: usize) -> Vec<Span<'static>> {
    let spans = incoming_spans(incoming, width);
    if incoming.stale {
        spans
            .into_iter()
            .map(|s| Span::styled(s.content, style(Role::Muted)))
            .collect()
    } else {
        spans
    }
}

/// The character's resources as tokens: orbs, Sovereign Blades.
fn resource_tokens(f: &mut Flow, view: &View<'_>) {
    let you = view.you;
    if you.orb_slots > 0 || !view.orbs.is_empty() {
        f.token(Span::styled("Orbs", style(Role::Muted)), 0, None);
        let slots = you.orb_slots.max(view.orbs.len());
        for slot in 0..slots {
            let target = Target::Orb(slot);
            f.token(
                Span::styled(
                    orb_text(slot, view.orbs.get(slot).copied()),
                    style(Role::Buff),
                ),
                if slot == 0 { 1 } else { 2 },
                Some(target),
            );
        }
    }
    for blade in &you.blades {
        let times = if blade.repeats > 1 {
            format!(" ×{}", blade.repeats)
        } else {
            String::new()
        };
        let target = Target::InfoCard(CardId(blade.card));
        f.token(
            Span::styled(
                format!("Sovereign Blade {}{times} ({})", blade.damage, blade.place),
                style(Role::Energy),
            ),
            2,
            Some(target),
        );
    }
}

/// `›Lightning 3/8`: an orb's passive and evoke values now (the first is evoked next).
fn orb_text(slot: usize, orb: Option<&OrbRow>) -> String {
    orb.map_or_else(
        || "·".to_owned(),
        |orb| {
            let next = if slot == 0 { "›" } else { "" };
            format!("{next}{} {}/{}", orb.name, orb.passive, orb.evoke)
        },
    )
}

/// The resources' flow `width` wide.
fn resources(view: &View<'_>, width: u16) -> Flow {
    let mut f = Flow::new(width);
    resource_tokens(&mut f, view);
    f
}

/// The rows of the resources: none without any.
fn resource_rows(view: &View<'_>, width: u16) -> u16 {
    let f = resources(view, width);
    if f.width_left() == f.width() && f.rows() == 1 {
        0
    } else {
        flow::coord(f.rows())
    }
}

/// The rows of the name, the HP bar and the incoming line: the bar two rows on a big screen.
const fn core_rows(view: &View<'_>, big: Big) -> u16 {
    let bars = if big.thick { 2 } else { 1 };
    let incoming = view.incoming.is_some() || view.choice.is_some();
    1 + bars + has_osty(view) as u16 + incoming as u16
}

/// What a card choice adds to the incoming line: it is from before the play.
const STALE_NOTE: &str = " (before this play)";

/// The player's HP bar `width` wide.
fn bar_width(view: &View<'_>, width: u16) -> usize {
    let widest = if scale::big() {
        3 * MAX_BAR
    } else if scale::wide() {
        2 * MAX_BAR
    } else {
        MAX_BAR
    };
    let numbers = hp_numbers_width(view.you.max_hp);
    let block = block_span(view.you.block, None).map_or(0, |b| 2 + flow::width(&b.content));
    usize::from(width)
        .saturating_sub(numbers + block)
        .clamp(4, widest)
}

/// Whether the fight has Osty to show (the Necrobinder's, alive or not).
const fn has_osty(view: &View<'_>) -> bool {
    view.you.pet_slot || !view.pets.is_empty()
}

/// The rows the player takes `width` wide: the name, the HP, the incoming line while the
/// player can act, the powers, the resources.
pub fn height(view: &View<'_>, plan: Plan, big: Big, width: u16) -> u16 {
    core_rows(view, big)
        + u16::from(plan.powers() && has_powers(&view.you.powers))
        + resource_rows(view, width)
}

/// Osty's row under the player's bar, `Osty ██████░░ 12/12  ▣ 3`, its bar `bar` cells wide: a
/// target for the cards aimed at it (the name in the keys' colour while it can be picked,
/// marked and reversed while the focus rests on it, the marker's cell kept blank otherwise so
/// it stays put); `Osty ✗`, dim, while it is dead.
fn osty_spans(view: &View<'_>, bar: usize, hits: &Hits) -> (Vec<Span<'static>>, Option<Target>) {
    let Some(osty) = view.pets.first().filter(|p| p.alive) else {
        return (vec![Span::styled(" Osty ✗", style(Role::Muted))], None);
    };
    let target = Target::Creature(CreatureId(osty.id));
    let focused = hits.focused(target);
    let name = if focused {
        style(Role::Selected)
    } else if osty.target.is_some() {
        style(Role::Key)
    } else {
        style(Role::Heal)
    };
    let marker = if focused { "▸" } else { " " };
    let mut spans = vec![Span::styled(format!("{marker}Osty"), name), Span::raw(" ")];
    spans.extend(hp_spans(
        (osty.hp, osty.max_hp, osty.alive),
        bar,
        Role::Heal,
        None,
        BarRow::Single,
    ));
    if let Some(block) = block_span(osty.block, None) {
        spans.push(Span::raw("  "));
        spans.push(block);
    }
    (spans, Some(target))
}

/// The name row: the character's name.
fn draw_name_row(frame: &mut Frame<'_>, name_row: Rect, view: &View<'_>) {
    let at = super::centred(
        frame,
        name_row,
        &[Span::styled(view.you.name.clone(), style(Role::Title))],
    );
    super::draw_hit(frame, name_row, at, view.you.id);
}

/// Draws the player from the top of `stack`.
pub fn draw(
    frame: &mut Frame<'_>,
    stack: &mut Stack,
    view: &View<'_>,
    plan: Plan,
    big: Big,
    hits: &mut Hits,
) {
    let you = view.you;
    let me = CreatureId(you.id);
    let whole = stack.take(height(view, plan, big, stack.rest().width));
    if whole.is_empty() {
        return;
    }
    hits.push(whole, Target::Creature(me));
    let area = whole;
    let mut rows = Stack::new(area);
    draw_name_row(frame, rows.take(1), view);
    let mut after = Vec::new();
    if let Some(block) = block_span(you.block, None) {
        after.push(Span::raw("  "));
        after.push(block);
    }
    let bar = bar_width(view, area.width);
    let you_hp = (you.hp, you.max_hp, true);
    // The HP the enemy turn takes, tinted into the bar; the numbers stay the HP you have.
    let taken = view
        .incoming
        .filter(|i| !i.stale && i.hp_loss > 0)
        .map(|i| (i.hp_after.max(0), theme::INCOMING));
    let first = if big.thick {
        BarRow::Top
    } else {
        BarRow::Single
    };
    // A hit drains first; the enemies' turn shows again once it rests.
    let drain = |row| super::draining_bar((you.id, you.hp, you.max_hp, true), bar, Role::Hp, row);
    let mut hp = drain(first).unwrap_or_else(|| tinted_bar(you_hp, bar, Role::Hp, taken, first));
    hp.push(numbers_span(you.hp, you.max_hp, false));
    // A thick bar closes its outline on a second row; the numbers stay on the first.
    let second = drain(BarRow::Bottom)
        .unwrap_or_else(|| tinted_bar(you_hp, bar, Role::Hp, taken, BarRow::Bottom));
    // Centred like the enemies' bars on the bar and its numbers alone, so Block appearing at
    // the right moves nothing; the second thick row sits under the first.
    let hp_width: usize = hp.iter().map(|s| flow::width(&s.content)).sum();
    let left = flow::coord(usize::from(area.width).saturating_sub(hp_width) / 2);
    hp.extend(after);
    let hp = flow::fit(&hp, usize::from(area.width) - usize::from(left));
    let beside = |row: Rect| Rect {
        x: row.x + left,
        width: row.width - left,
        ..row
    };
    frame.render_widget(Paragraph::new(Line::from(hp)), beside(rows.take(1)));
    if big.thick {
        frame.render_widget(Paragraph::new(Line::from(second)), beside(rows.take(1)));
    }
    if has_osty(view) {
        let row = rows.take(1);
        let (spans, target) = osty_spans(view, (bar / 2).max(4), hits);
        let at = super::centred(frame, row, &spans);
        if let Some(target) = target {
            hits.push(at, target);
        }
    }
    if let Some(incoming) = view.incoming {
        let width = usize::from(area.width);
        let mut spans = incoming_words(incoming, width);
        // A card choice shows the line as the turn's last play left it, noted.
        if incoming.stale {
            let used: usize = spans.iter().map(|s| flow::width(&s.content)).sum();
            if used + flow::width(STALE_NOTE) <= width {
                spans.push(Span::styled(STALE_NOTE, style(Role::Muted)));
            }
        }
        super::centred(frame, rows.take(1), &spans);
    }
    // A choice with no incoming line to keep holds its row, so the rows below stay put.
    if view.incoming.is_none() && view.choice.is_some() {
        let spans = [Span::styled(
            "Incoming: after your choice",
            style(Role::Muted),
        )];
        super::centred(frame, rows.take(1), &spans);
    }
    if plan.powers() && has_powers(&you.powers) {
        let row = rows.take(1);
        let mut f = Flow::new(row.width);
        power_tokens(&mut f, you.id, &you.powers, false);
        let lines = f.finish(row, hits);
        frame.render_widget(Paragraph::new(lines), row);
    }
    let rest = rows.rest();
    if !rest.is_empty() {
        let lines = resources(view, rest.width).finish(rest, hits);
        frame.render_widget(Paragraph::new(lines), rest);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn words(incoming: &IncomingRow, width: usize) -> String {
        incoming_spans(incoming, width)
            .iter()
            .map(|s| s.content.as_ref())
            .collect()
    }

    fn preview(total: i32, blocked: i32, hp_loss: i32) -> IncomingRow {
        IncomingRow {
            total,
            blocked,
            absorbed: 0,
            hp_loss,
            hp_after: 80 - hp_loss,
            lethal: false,
            certain: true,
            stale: false,
        }
    }

    #[test]
    fn the_incoming_line_says_the_hp_the_turn_takes() {
        assert_eq!(words(&preview(7, 0, 7), 43), "⚔ −7 HP next turn");
        assert_eq!(
            words(&preview(12, 5, 7), 43),
            "⚔ −7 HP next turn",
            "after the Block"
        );
        assert_eq!(words(&preview(12, 12, 0), 43), "▣ All blocked next turn");
        assert_eq!(words(&preview(0, 0, 0), 43), "No incoming damage");
        let osty = IncomingRow {
            absorbed: 4,
            ..preview(12, 5, 3)
        };
        assert_eq!(words(&osty, 43), "⚔ −3 HP next turn", "after Osty");
        let unsure = IncomingRow {
            certain: false,
            ..preview(7, 0, 7)
        };
        assert_eq!(words(&unsure, 43), "⚔ −7 HP or more next turn");
        let unsure = IncomingRow {
            certain: false,
            ..preview(0, 0, 0)
        };
        assert_eq!(words(&unsure, 43), "⚔ next turn: unknown");
        let lethal = IncomingRow {
            lethal: true,
            hp_after: 0,
            ..preview(95, 5, 80)
        };
        assert_eq!(words(&lethal, 43), "☠ −80 HP next turn   LETHAL ");
    }
}
