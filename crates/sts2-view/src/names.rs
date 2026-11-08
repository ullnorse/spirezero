//! Display names and log lines for engine values.

use sts2_core::combat::state::Card;
use sts2_core::combat::{CardId, CombatState, CreatureId, DeckCard, Event, Side};
use sts2_core::content::cards;
use sts2_core::content::enchantments::{Enchantment, EnchantmentKind, EnchantmentStatus};
use sts2_core::content::powers::PowerKind;
use sts2_core::content::relics::RelicKind;
use sts2_core::run::potions::combat_potion;
use sts2_core::run::{MapPointType, RunCard, RunError, RunState};

/// `SCREAMING_SNAKE` to `Title Case`.
#[must_use]
pub fn title_case(entry: &str) -> String {
    entry
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first
                    .to_uppercase()
                    .chain(chars.flat_map(char::to_lowercase))
                    .collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A card's display name, with `+` when upgraded.
#[must_use]
pub fn card(state: &CombatState, id: CardId) -> String {
    let card = state.card(id);
    let plus = if card.is_upgraded() { "+" } else { "" };
    format!("{}{plus}", card.kind.name())
}

/// Whether the game prints an enchantment's amount on the card (`EnchantmentModel.ShowAmount`,
/// true for these six; `NCard.cs:922`).
const fn shows_amount(kind: EnchantmentKind) -> bool {
    matches!(
        kind,
        EnchantmentKind::Momentum
            | EnchantmentKind::Adroit
            | EnchantmentKind::Sharp
            | EnchantmentKind::Nimble
            | EnchantmentKind::Vigorous
            | EnchantmentKind::Swift
    )
}

/// The tag of an enchantment on a card, `✦Sharp 2` (the amount only where the game shows it,
/// `spent` for a disabled one).
#[must_use]
pub fn enchantment(enchantment: Enchantment) -> String {
    let amount = if shows_amount(enchantment.kind) {
        format!(" {}", enchantment.amount)
    } else {
        String::new()
    };
    let spent = match enchantment.status {
        EnchantmentStatus::Normal => "",
        EnchantmentStatus::Disabled => " (spent)",
    };
    format!("✦{}{amount}{spent}", enchantment.kind.name())
}

/// `name` followed by the enchantment's tag, if the card has one.
fn tagged(name: String, on_card: Option<Enchantment>) -> String {
    match on_card {
        Some(e) => format!("{name} {}", enchantment(e)),
        None => name,
    }
}

/// The enchantment tag of a card of the fight.
#[must_use]
pub fn card_tag(state: &CombatState, id: CardId) -> Option<String> {
    state.card(id).enchantment.map(enchantment)
}

/// The cards in a list, comma separated.
#[must_use]
pub fn cards(state: &CombatState, ids: &[CardId]) -> String {
    ids.iter()
        .map(|&id| card(state, id))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A creature's display name.
#[must_use]
pub fn creature(state: &CombatState, id: CreatureId) -> String {
    state
        .creature(id)
        .monster
        .as_ref()
        .map_or("You", |m| m.kind.name())
        .to_owned()
}

/// A power's display name.
#[must_use]
pub fn power(kind: PowerKind) -> &'static str {
    kind.name()
}

/// `verb` for the subject `who`: "You hit", "Jaw Worm hits" (regular verbs).
fn verb(who: &str, verb: &str) -> String {
    if who == "You" {
        verb.to_owned()
    } else {
        format!("{verb}s")
    }
}

/// A monster move id as a display name.
#[must_use]
pub fn monster_move(name: &str) -> String {
    // Move ids end in `_MOVE` (`OIL_SPRAY_MOVE`): the suffix is noise on screen.
    let name = title_case(name);
    name.strip_suffix(" Move")
        .map_or_else(|| name.clone(), str::to_owned)
}

/// One log line per event, or `None` for events not worth showing.
#[must_use]
pub fn event_line(state: &CombatState, event: &Event) -> Option<String> {
    Some(match event {
        Event::TurnStarted { side, round } => match side {
            Side::Player => format!("── Round {round}: your turn ──"),
            Side::Enemy => "── Enemy turn ──".to_owned(),
        },
        Event::Shuffled => "Draw pile reshuffled".to_owned(),
        Event::CardDrawn(id) => format!("Drew {}", card(state, *id)),
        Event::CardPlayed { card: id, target } => target.map_or_else(
            || format!("Played {}", card(state, *id)),
            |t| format!("Played {} on {}", card(state, *id), creature(state, t)),
        ),
        Event::HandFlushed(ids) => format!("Discarded hand ({} cards)", ids.len()),
        Event::PotionUsed { potion, target } => target.map_or_else(
            || format!("Used {}", potion.name()),
            |t| format!("Used {} on {}", potion.name(), creature(state, t)),
        ),
        Event::Attack { .. } => return None,
        Event::MonsterMove { monster, name } => {
            format!("{} uses {}", creature(state, *monster), monster_move(name))
        }
        Event::Damage {
            dealer,
            receiver,
            blocked,
            hp_lost,
        } => {
            let who = dealer.map_or_else(|| "Something".to_owned(), |d| creature(state, d));
            format!(
                "{who} {} {}: {hp_lost} damage, {blocked} blocked",
                verb(&who, "hit"),
                creature(state, *receiver)
            )
        }
        Event::BlockGained {
            creature: c,
            amount,
        } => {
            let who = creature(state, *c);
            format!("{who} {} {amount} block", verb(&who, "gain"))
        }
        Event::PowerChanged {
            creature: c,
            kind,
            amount,
        } => format!("{} {} {amount:+}", creature(state, *c), power(*kind)),
        Event::Summoned { amount } => format!("Summoned Osty ({amount})"),
        Event::Died(c) => {
            let who = creature(state, *c);
            format!("{who} {}", verb(&who, "die"))
        }
        Event::Escaped(c) => {
            let who = creature(state, *c);
            format!("{who} {}", verb(&who, "escape"))
        }
        Event::Won => "Victory!".to_owned(),
        Event::Lost => "Defeat.".to_owned(),
    })
}

/// A deck or reward card's name, with `+` per upgrade level (the id in title case if the combat
/// engine does not port it).
#[must_use]
pub fn run_card(card: RunCard) -> String {
    let name = card
        .to_combat()
        .map_or_else(|| title_case(card.id), |c| c.kind.name().to_owned());
    let name = match card.upgrade_level {
        0 => name,
        1 => format!("{name}+"),
        n => format!("{name}+{n}"),
    };
    tagged(name, card.enchantment)
}

/// A deck card as the fight would clone it, with its saved fields (`MadScience` keeps its
/// `TinkerTimeType` there) and its upgrades applied.
fn deck_card(c: DeckCard) -> Card {
    let mut card = Card::new(CardId(0), c.kind, c.upgrade_level);
    card.data = cards::data_from_deck(c.kind, c.counter);
    card
}

/// [`run_card`] with its cost, type and rarity.
#[must_use]
pub fn run_card_detail(card: RunCard) -> String {
    card.to_combat().map_or_else(
        || run_card(card),
        |c| {
            let fight_card = deck_card(c);
            let cost = if fight_card.energy_cost.costs_x {
                "X".to_owned()
            } else {
                fight_card.energy_cost.base.to_string()
            };
            format!(
                "{} ({:?}, {:?}, {cost} energy)",
                run_card(card),
                fight_card.card_type(),
                c.kind.rarity(),
            )
        },
    )
}

/// A deck, reward or shop card's energy cost (`X` for an X cost), if the engine ports it.
#[must_use]
pub fn run_card_cost(card: RunCard) -> Option<String> {
    let c = deck_card(card.to_combat()?);
    let energy = if c.energy_cost.costs_x {
        "X".to_owned()
    } else if c.energy_cost.base < 0 {
        "—".to_owned()
    } else {
        c.energy_cost.base.to_string()
    };
    let stars = if c.star_cost.costs_x {
        "·sX".to_owned()
    } else if c.star_cost.base < 0 {
        String::new()
    } else {
        format!("·s{}", c.star_cost.base)
    };
    Some(format!("{energy}{stars}"))
}

/// A relic's display name from its id.
#[must_use]
pub fn relic(id: &str) -> String {
    RelicKind::from_id(id).map_or_else(|| title_case(id), |k| k.name().to_owned())
}

/// A potion's display name from its id.
#[must_use]
pub fn potion(id: &str) -> String {
    combat_potion(id).map_or_else(|| title_case(id), |k| k.name().to_owned())
}

/// A map point's display name; the ancient's is the current act's ancient (`Neow` in Act 1).
#[must_use]
pub fn point_type(state: &RunState, kind: MapPointType) -> String {
    let name = match kind {
        MapPointType::Unassigned => "Unassigned",
        MapPointType::Unknown => "Unknown (?)",
        MapPointType::Shop => "Shop",
        MapPointType::Treasure => "Treasure",
        MapPointType::RestSite => "Rest site",
        MapPointType::Monster => "Monster",
        MapPointType::Elite => "Elite",
        MapPointType::Boss => "Boss",
        MapPointType::Ancient => return title_case(state.act().ancient),
    };
    name.to_owned()
}

/// An event option's `TextKey` (`EVENT.pages.PAGE.options.OPTION`) as readable text.
#[must_use]
pub fn event_option(text_key: &str) -> String {
    title_case(text_key.rsplit('.').next().unwrap_or(text_key))
}

/// An event page's name from an option's `TextKey`, if it has one.
#[must_use]
pub fn event_page(text_key: &str) -> Option<String> {
    let mut parts = text_key.split('.');
    parts.next()?;
    (parts.next()? == "pages").then_some(())?;
    parts.next().map(title_case)
}

/// What an engine error says, with unported content called out as such.
#[must_use]
pub fn run_error(error: &RunError) -> String {
    let content = match error {
        RunError::UnsupportedEncounter(id) => Some(format!("encounter {id}")),
        RunError::UnsupportedAncientOption(id) => Some(format!("Neow option {id}")),
        RunError::UnsupportedRelic(id) => Some(format!("relic {id}")),
        RunError::UnsupportedCard(id) => Some(format!("card {id}")),
        RunError::UnsupportedPotion(id) => Some(format!("potion {id}")),
        RunError::UnsupportedConfig(what) => Some(what.to_string()),
        _ => None,
    };
    content.map_or_else(
        || error.to_string(),
        |what| format!("this content is not ported yet: {what}"),
    )
}

/// An act's display name from its `ModelId.Entry` (`OVERGROWTH`).
#[must_use]
pub fn act(id: &str) -> String {
    title_case(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_lines_and_moves_read_as_english() {
        assert_eq!(verb("You", "hit"), "hit");
        assert_eq!(verb("Jaw Worm", "hit"), "hits");
        assert_eq!(verb("Jaw Worm", "die"), "dies");
        assert_eq!(monster_move("OIL_SPRAY_MOVE"), "Oil Spray");
        assert_eq!(monster_move("SLAM"), "Slam");
    }

    #[test]
    fn an_upgrade_that_lowers_the_cost_shows_in_the_detail() {
        let mut card = RunCard::new("SEVEN_STARS");
        assert!(run_card_detail(card).contains("2 energy"));
        card.upgrade_level = 1;
        assert!(run_card_detail(card).contains("1 energy"));
    }
}
