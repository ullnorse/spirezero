//! The detail sheet's text for one object (spec 2.14).
//!
//! A card in full with its upgrade and the keyword glossary, an enemy's intent, powers and
//! moves in words. Each is a [`DetailSheet`] of [`Rich`] lines; a client draws it as a sheet
//! over the column or as the wide layout's panel.

use sts2_core::Combat;
use sts2_core::combat::{CardId, CombatState, CreatureId, Event, Intent, IntentType, PileType};
use sts2_core::content::cards::Keywords;
use sts2_core::decimal::Decimal;
use sts2_core::run::RunCard;

use crate::combat::{cost_label, type_word};
use crate::describe::{Describer, Term};
use crate::model::{DetailSheet, MapCell, MapKind, MapRange};
use crate::names;
use crate::text::{self, Rich, RichSpan, Role};

fn line(role: Role, text: impl Into<String>) -> Rich {
    Rich(vec![RichSpan {
        fx: None,
        role,
        bold: false,
        text: text.into(),
    }])
}

/// The glossary: each term's title, then its description.
fn glossary(terms: Vec<Term>, lines: &mut Vec<Rich>) {
    for term in terms {
        lines.push(Rich::default());
        lines.push(line(Role::Buff, term.title));
        lines.extend(text::lines(&term.text, Role::Muted));
    }
}

/// `Upgraded: ...`: the text one upgrade up, if the card can be upgraded.
fn upgraded(describer: &Describer, card: RunCard, lines: &mut Vec<Rich>) {
    if let Some(up) = describer.upgraded_run_card(card) {
        let mut spans = vec![RichSpan {
            fx: None,
            role: Role::Muted,
            bold: false,
            text: "Upgraded: ".to_owned(),
        }];
        spans.extend(text::inline(&up, Role::Plain).0);
        lines.push(Rich::default());
        lines.push(Rich(spans));
    }
}

/// A card of the fight: type, cost and rarity, its text with its live values (or why it cannot
/// be played), the upgraded text, the glossary.
#[must_use]
pub fn card(describer: &Describer, state: &CombatState, id: CardId) -> DetailSheet {
    let c = state.card(id);
    let mut lines = vec![line(
        Role::Muted,
        format!(
            "{} · {} energy · {:?}",
            type_word(c.card_type()),
            cost_label(state, id),
            c.kind.rarity()
        ),
    )];
    if let Some(tag) = names::card_tag(state, id) {
        lines.push(line(Role::Debuff, tag));
    }
    if state.pile_of(id) == Some(PileType::Hand)
        && let Some(reason) = state.unplayable_reason(id)
    {
        lines.extend(text::lines(
            &describer.unplayable(state, reason),
            Role::Emphasis,
        ));
    }
    let markup = describer.hand_card(state, id);
    match &markup {
        Some(m) => lines.extend(text::lines(m, Role::Plain)),
        None => lines.push(line(
            Role::Plain,
            crate::describe::vars_text(state, id, None),
        )),
    }
    let mut run_card = RunCard::new(c.kind.entry());
    run_card.upgrade_level = c.upgrade_level;
    upgraded(describer, run_card, &mut lines);
    glossary(
        describer.glossary(markup.as_deref().unwrap_or_default(), state.keywords(id)),
        &mut lines,
    );
    DetailSheet {
        title: line(Role::Title, names::card(state, id)),
        lines,
        ..DetailSheet::default()
    }
}

/// A deck, reward or shop card: its type, rarity and cost, text, upgraded text, glossary.
#[must_use]
pub fn run_card(describer: &Describer, card: RunCard) -> DetailSheet {
    let name = names::run_card(card);
    let detail = names::run_card_detail(card);
    let kind = detail
        .strip_prefix(&name)
        .map(|rest| {
            rest.trim()
                .trim_matches(|c| c == '(' || c == ')')
                .to_owned()
        })
        .filter(|rest| !rest.is_empty());
    let mut lines: Vec<Rich> = kind.map(|k| line(Role::Muted, k)).into_iter().collect();
    let markup = describer.run_card(card);
    if let Some(m) = &markup {
        lines.extend(text::lines(m, Role::Plain));
    }
    upgraded(describer, card, &mut lines);
    glossary(
        describer.glossary(markup.as_deref().unwrap_or_default(), Keywords::default()),
        &mut lines,
    );
    DetailSheet {
        title: line(Role::Title, name),
        lines,
        ..DetailSheet::default()
    }
}

/// What an intent means, in words.
#[must_use]
pub fn intent_text(intent: &Intent) -> String {
    let attack = intent.attack.map(|a| {
        let hits = if a.hits > 1 {
            format!(" {} times", a.hits)
        } else {
            String::new()
        };
        format!("{} damage{hits}", a.damage)
    });
    match intent.kind {
        IntentType::Attack => format!(
            "Intends to attack for {}.",
            attack.unwrap_or_else(|| "unknown damage".to_owned())
        ),
        IntentType::DeathBlow => format!(
            "Intends a death blow: {}.",
            attack.unwrap_or_else(|| "unknown damage".to_owned())
        ),
        IntentType::Buff => "Intends to use a buff.".to_owned(),
        IntentType::Debuff => "Intends to inflict a negative effect.".to_owned(),
        IntentType::DebuffStrong => "Intends to inflict a strong negative effect.".to_owned(),
        IntentType::Defend => "Intends to gain Block.".to_owned(),
        IntentType::Escape => "Intends to escape.".to_owned(),
        IntentType::Heal => "Intends to heal.".to_owned(),
        IntentType::Hidden | IntentType::Unknown => "Its intent is unknown.".to_owned(),
        IntentType::Summon => "Intends to summon.".to_owned(),
        IntentType::Sleep => "Is asleep.".to_owned(),
        IntentType::Stun => "Is stunned.".to_owned(),
        IntentType::StatusCard => format!(
            "Intends to add {} status card(s) to your cards.",
            intent.cards.unwrap_or(0)
        ),
        IntentType::CardDebuff => "Intends to weaken your cards.".to_owned(),
    }
}

/// An enemy: HP and Block, its intent in words, its powers with their text, and its moves so
/// far this fight.
#[must_use]
pub fn enemy(describer: &Describer, combat: &Combat, id: CreatureId) -> DetailSheet {
    let state = combat.state();
    let c = state.creature(id);
    let name = names::creature(state, id);
    let mut lines = vec![line(
        Role::Muted,
        if c.is_alive() {
            format!("HP {}/{} · Block {}", c.hp, c.max_hp, c.block)
        } else {
            "dead".to_owned()
        },
    )];
    if let Some(m) = c.monster.as_ref().filter(|_| c.is_alive()) {
        lines.push(Rich(vec![
            RichSpan {
                fx: None,
                role: Role::Muted,
                bold: false,
                text: "Next: ".to_owned(),
            },
            RichSpan {
                fx: None,
                role: Role::Emphasis,
                bold: false,
                text: names::monster_move(m.next_move_name()),
            },
        ]));
        lines.extend(
            state
                .intents(id)
                .iter()
                .map(|i| line(Role::Plain, intent_text(i))),
        );
    }
    for p in c.powers.iter().filter(|p| p.kind.is_visible()) {
        lines.push(Rich::default());
        let role = match p.kind.type_for_amount(Decimal::from(p.amount)) {
            sts2_core::content::powers::PowerType::Buff => Role::Buff,
            sts2_core::content::powers::PowerType::Debuff => Role::Debuff,
        };
        lines.push(line(role, format!("{} {}", names::power(p.kind), p.amount)));
        if let Some(markup) =
            describer.power(p.kind, p.amount, state.power_data(p.id), &name, false)
        {
            lines.extend(text::lines(&markup, Role::Plain));
        }
    }
    let moves: Vec<String> = combat
        .events()
        .iter()
        .filter_map(|e| match e {
            Event::MonsterMove { monster, name } if *monster == id => {
                Some(names::monster_move(name))
            }
            _ => None,
        })
        .collect();
    if !moves.is_empty() {
        lines.push(Rich::default());
        lines.push(Rich(vec![
            RichSpan {
                fx: None,
                role: Role::Muted,
                bold: false,
                text: "Moves so far: ".to_owned(),
            },
            RichSpan {
                fx: None,
                role: Role::Plain,
                bold: false,
                text: moves.join(", "),
            },
        ]));
    }
    DetailSheet {
        title: line(Role::Title, name),
        lines,
        ..DetailSheet::default()
    }
}

/// A named thing with text: a relic, a potion, a menu entry.
#[must_use]
pub fn text_of(title: &str, markup_lines: &[Rich]) -> DetailSheet {
    DetailSheet {
        title: line(Role::Title, title),
        lines: markup_lines.to_vec(),
        ..DetailSheet::default()
    }
}

/// What a room kind is, in a line (`MapCell::room`'s key).
fn room_blurb(room: &str) -> &'static str {
    match room {
        "Monster" => "A fight with ordinary enemies.",
        "Elite" => "A hard fight; it pays a relic.",
        "Unknown" => "Not known until you enter: a fight, an event, a shop or a treasure.",
        "Shop" => "Spend gold on cards, relics and potions, or remove a card.",
        "Treasure" => "A chest with a relic.",
        "RestSite" => "Rest to heal, or smith to upgrade a card.",
        "Boss" => "The act's boss.",
        _ => "",
    }
}

/// A room of the map the player can travel to: what it is, and what every path through it to
/// the boss still passes, kind by kind (`3` where every path agrees, `1–3` where they differ).
#[must_use]
pub fn map_room(cell: &MapCell, kinds: &[MapKind]) -> DetailSheet {
    let word = kinds
        .iter()
        .find(|k| k.room == cell.room)
        .map_or_else(|| cell.room.clone(), |k| k.word.clone());
    let mut title = word;
    if let Some(first) = title.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    let mut lines = vec![line(Role::Plain, room_blurb(&cell.room))];
    let ahead: Vec<(&MapKind, MapRange)> = kinds
        .iter()
        .zip(&cell.through)
        .filter(|(_, r)| r.max > 0)
        .map(|(k, r)| (k, *r))
        .collect();
    if !ahead.is_empty() {
        lines.push(Rich::default());
        lines.push(line(
            Role::Muted,
            "On the paths from here to the boss, this room counted:",
        ));
        for (kind, range) in ahead {
            let count = if range.min == range.max {
                range.max.to_string()
            } else {
                format!("{}–{}", range.min, range.max)
            };
            lines.push(line(kind.role, format!("{count}  {}", kind.word)));
        }
    }
    DetailSheet {
        title: line(Role::Title, format!("{title} room")),
        lines,
        ..DetailSheet::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_map_room_says_what_it_is_and_what_the_paths_through_it_pass() {
        let kind = |room: &str, glyph: char, word: &str| MapKind {
            room: room.to_owned(),
            glyph,
            word: word.to_owned(),
            ..MapKind::default()
        };
        let kinds = vec![
            kind("Monster", 'M', "monster"),
            kind("RestSite", 'R', "rest"),
            kind("Elite", 'E', "elite"),
        ];
        let cell = MapCell {
            room: "Monster".to_owned(),
            through: vec![
                MapRange { min: 4, max: 4 },
                MapRange { min: 1, max: 3 },
                MapRange { min: 0, max: 0 },
            ],
            ..MapCell::default()
        };
        let sheet = map_room(&cell, &kinds);
        assert_eq!(sheet.title.text(), "Monster room");
        let lines: Vec<String> = sheet.lines.iter().map(Rich::text).collect();
        assert_eq!(lines[0], "A fight with ordinary enemies.");
        assert!(lines.contains(&"4  monster".to_owned()), "{lines:?}");
        assert!(lines.contains(&"1–3  rest".to_owned()), "{lines:?}");
        assert!(
            !lines.iter().any(|l| l.contains("elite")),
            "a kind no path passes is left out: {lines:?}"
        );
    }
}
