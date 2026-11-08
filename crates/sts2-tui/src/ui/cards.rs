//! How a card is worded, the same everywhere: its cost, its type, and a card as one row of a
//! list (cost, name, its whole text on one line, sentences joined by ` · `, cut with `…`; its
//! text in full while the focus rests on it).

use ratatui::style::Modifier;
use ratatui::text::Span;

use crate::ui::button::Expanded;
use crate::ui::flow;
use crate::ui::scale;
use crate::ui::theme::{self, Role, style};

/// The widest a card's name column gets; on a wide column, [`WIDE_NAME_WIDTH`].
const NAME_WIDTH: usize = 14;
const WIDE_NAME_WIDTH: usize = 22;

/// A card as a list shows it: its cost, its name, its whole text on one line (the keywords in
/// their colours), and its text in full for while the focus rests on it.
pub struct Row {
    pub cost: String,
    pub name: String,
    /// The card's type as a word (`Attack`), shown on a wide column.
    pub kind: String,
    /// Its rarity (`Common`, `Rare`, ...): the name in its colour, and beside the type on a
    /// wide column; empty when unknown.
    pub rarity: String,
    pub effect: Vec<Span<'static>>,
    pub text: Vec<Span<'static>>,
    /// A hand card's gold glow is on: a gold `✦` after the name, in its padding.
    pub glow: bool,
}

/// `rows` as list rows, their costs and names in aligned columns: the cost behind the energy
/// glyph in yellow (`ϟ1`, so it is never a bare digit like the key), the name bold, on a wide
/// column the type dim, the effect grey; and each in full ([`Expanded`]), the text in the
/// effect's column, or with `tall` on a second row under the name (a big screen's card rows).
#[must_use]
pub fn spans(rows: &[Row], tall: bool) -> Vec<(Vec<Span<'static>>, Expanded)> {
    let wide = scale::wide();
    // The type, with the rarity worth saying: `Attack · Rare`.
    let kind_word = |r: &Row| {
        if theme::rarity_shown(&r.rarity) {
            format!("{} · {}", r.kind, r.rarity)
        } else {
            r.kind.clone()
        }
    };
    let kind_width = if wide {
        rows.iter()
            .map(|r| flow::width(&kind_word(r)))
            .max()
            .unwrap_or(0)
    } else {
        0
    };
    let costs: Vec<String> = rows
        .iter()
        .map(|r| {
            if r.cost.is_empty() {
                String::new()
            } else {
                format!("ϟ{}", r.cost)
            }
        })
        .collect();
    let cost_width = costs.iter().map(|c| flow::width(c)).max().unwrap_or(0);
    let name_width = rows
        .iter()
        .map(|r| flow::width(&r.name))
        .max()
        .unwrap_or(0)
        .min(if wide { WIDE_NAME_WIDTH } else { NAME_WIDTH });
    rows.iter()
        .zip(&costs)
        .map(|(r, cost)| {
            let name = flow::truncated(&r.name, name_width);
            let mut prefix = vec![
                Span::styled(format!("{cost:<cost_width$} "), style(Role::Energy)),
                Span::styled(
                    format!("{name:<name_width$}"),
                    theme::rarity(&r.rarity)
                        .unwrap_or_default()
                        .add_modifier(Modifier::BOLD),
                ),
                if r.glow {
                    Span::styled("✦ ", style(Role::Gold))
                } else {
                    Span::raw("  ")
                },
            ];
            if kind_width > 0 {
                let pad = kind_width - flow::width(&kind_word(r));
                prefix.push(Span::styled(r.kind.clone(), style(Role::Muted)));
                if theme::rarity_shown(&r.rarity) {
                    prefix.push(Span::styled(" · ", style(Role::Muted)));
                    prefix.push(Span::styled(
                        r.rarity.clone(),
                        theme::rarity(&r.rarity).unwrap_or_else(|| style(Role::Muted)),
                    ));
                }
                prefix.push(Span::raw(" ".repeat(pad + 2)));
            }
            let mut line = prefix.clone();
            // A card without text shows its type as its effect; not twice beside the type column.
            let effect_words: String = r.effect.iter().map(|s| s.content.as_ref()).collect();
            let repeats = kind_width > 0 && effect_words == r.kind;
            if !repeats {
                line.extend(r.effect.iter().cloned());
            }
            (
                line,
                Expanded {
                    prefix,
                    text: if repeats { Vec::new() } else { r.text.clone() },
                    // A card without text keeps to one row.
                    below: (tall && !repeats && !r.text.is_empty()).then_some(cost_width + 1),
                },
            )
        })
        .collect()
}
