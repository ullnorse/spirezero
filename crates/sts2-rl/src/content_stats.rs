//! A fixed numeric vector per card (base and upgraded), relic and potion vocab id: the content
//! prior the end-to-end net adds to each card, relic and potion token (RLN29, `card_stats`).
//!
//! Only what the content's text and frame show the player, read from our own content: a card's
//! type, rarity, target kind, energy and star costs (X, unplayable), keywords, tags, its
//! `DynamicVars` (damage, hits, block, draw, energy, the common powers, two more numbers) and
//! the upgrade's deltas; a potion's usage, target and vars; a relic's vars. The vars are matched
//! by the name the text shows (`DamageVar` is `Damage`, `PowerVar<WeakPower>` is `WeakPower`),
//! so the three kinds share their number columns.
//!
//! The tables ([`table`]) are `i16` rows of [`COLUMNS`], by vocab id ([`crate::vocab`]); a
//! card has a base and an upgraded table (the same row for a card that cannot upgrade). Every
//! row of a vocab id the engine lacks (`<pad>`, `<unk>`, unported content) is zero.
//! [`Column::magnitude`] marks the columns a reader should squash (the rest are 0/1 or -1/0/1).

use std::sync::OnceLock;

use sts2_core::combat::CardId;
use sts2_core::combat::state::Card;
use sts2_core::content::cards::{
    CardKeyword, CardKind, CardRarity, CardType, CardVar, TargetType, Var,
};
use sts2_core::content::potions::{PotionKind, PotionUsage};
use sts2_core::content::relics::RelicKind;
use sts2_core::decimal::Decimal;

use crate::vocab;

/// A column of the vector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Column {
    /// Its name (the FFI's column list).
    pub name: &'static str,
    /// A count or amount (squash it), not a flag.
    pub magnitude: bool,
}

/// The vars with their own column, by the name the text shows, in column order.
const VAR_COLUMNS: [&str; 29] = [
    "Damage",
    "Repeat",
    "Block",
    "Cards",
    "Energy",
    "Stars",
    "Summon",
    "OstyDamage",
    "HpLoss",
    "Forge",
    "Gold",
    "MaxHp",
    "Heal",
    "HealPercent",
    "CalculationBase",
    "CalculationExtra",
    "ExtraDamage",
    "Turns",
    "VulnerablePower",
    "WeakPower",
    "StrengthPower",
    "DexterityPower",
    "PoisonPower",
    "FocusPower",
    "DoomPower",
    "PlatingPower",
    "VigorPower",
    "ThornsPower",
    "IntangiblePower",
];

/// The flag columns, in order.
const FLAGS: [&str; 52] = [
    // What the row is.
    "is_card",
    "is_relic",
    "is_potion",
    // `CardType`.
    "attack",
    "skill",
    "power",
    "status",
    "curse",
    "quest",
    // `CardRarity`.
    "rarity_none",
    "basic",
    "common",
    "uncommon",
    "rare",
    "ancient",
    "event",
    "token",
    "rarity_status",
    "rarity_curse",
    "rarity_quest",
    // `TargetType` (cards and potions).
    "target_self",
    "target_none",
    "target_enemy",
    "target_all_enemies",
    "target_random_enemy",
    "target_player",
    // `PotionUsage`.
    "combat_only",
    "any_time",
    "automatic",
    // Keywords, in `CardKeyword` order.
    "exhaust",
    "ethereal",
    "innate",
    "unplayable",
    "retain",
    "sly",
    "eternal",
    // Costs.
    "cost_x",
    "star_cost_x",
    "has_star_cost",
    // Tags and text.
    "osty_attack",
    "strike",
    "defend",
    "shiv",
    "minion",
    "gains_block",
    "colorless",
    "turn_end_in_hand",
    "calculated_damage",
    "calculated_block",
    "calculated_other",
    // Upgrades.
    "upgradable",
    "upgraded",
];

/// The number columns after the vars, in order.
const NUMBERS_AFTER_VARS: [&str; 4] = ["power_other", "other_a", "other_b", "vars"];

/// The number columns: the costs, [`VAR_COLUMNS`], then [`NUMBERS_AFTER_VARS`].
const NUMBERS: usize = 2 + VAR_COLUMNS.len() + NUMBERS_AFTER_VARS.len();
/// The keyword delta columns.
const KEYWORDS: usize = 7;
/// Where the keyword flags start.
const KEYWORD_AT: usize = 29;
/// Where the number columns start.
const NUMBER_AT: usize = FLAGS.len();
/// Where the upgrade deltas of the number columns start, then the keywords'.
const DELTA_AT: usize = NUMBER_AT + NUMBERS;

/// The vector's width.
pub const WIDTH: usize = DELTA_AT + NUMBERS + KEYWORDS;

/// Every column, in order.
///
/// [`FLAGS`], the numbers (the energy and star costs, the vars, the first other power's
/// amount, the first two other numbers, the count of numeric vars), then the upgrade's deltas
/// (upgraded minus base) of every number and keyword.
#[must_use]
pub fn columns() -> &'static [Column] {
    static COLUMNS: OnceLock<Vec<Column>> = OnceLock::new();
    COLUMNS.get_or_init(|| {
        let flag = |name| Column {
            name,
            magnitude: false,
        };
        let number = |name| Column {
            name,
            magnitude: true,
        };
        let numbers: Vec<&'static str> = ["cost", "star_cost"]
            .into_iter()
            .chain(VAR_COLUMNS)
            .chain(NUMBERS_AFTER_VARS)
            .collect();
        let mut out: Vec<Column> = FLAGS.into_iter().map(flag).collect();
        out.extend(numbers.iter().copied().map(number));
        out.extend(numbers.iter().map(|n| number(leak(format!("d_{n}")))));
        out.extend(
            FLAGS[KEYWORD_AT..KEYWORD_AT + KEYWORDS]
                .iter()
                .map(|n| flag(leak(format!("d_{n}")))),
        );
        debug_assert_eq!(out.len(), WIDTH);
        out
    })
}

/// A column name built once ([`columns`] is built once per process).
fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// The column of `name`.
///
/// # Panics
/// If there is none.
#[must_use]
pub fn column(name: &str) -> usize {
    columns()
        .iter()
        .position(|c| c.name == name)
        .unwrap_or_else(|| panic!("no content-stats column {name}"))
}

/// A row.
pub type Row = [i16; WIDTH];

/// The content kinds with a table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Table {
    /// Cards, not upgraded.
    Cards,
    /// Cards, upgraded once.
    CardsUpgraded,
    /// Relics.
    Relics,
    /// Potions.
    Potions,
}

impl Table {
    /// From the FFI's number (0-3, in declaration order).
    #[must_use]
    pub const fn from_index(i: u32) -> Option<Self> {
        match i {
            0 => Some(Self::Cards),
            1 => Some(Self::CardsUpgraded),
            2 => Some(Self::Relics),
            3 => Some(Self::Potions),
            _ => None,
        }
    }
}

/// The rows of `table` by vocab id (as many as the vocab file has lines).
#[must_use]
pub fn table(table: Table) -> &'static [Row] {
    static TABLES: OnceLock<[Vec<Row>; 4]> = OnceLock::new();
    let all = TABLES.get_or_init(|| {
        let mut cards = vec![[0; WIDTH]; vocab::CARDS.ids().count()];
        let mut upgraded = cards.clone();
        for kind in CardKind::ALL {
            let id = usize::from(vocab::card(kind));
            if id > usize::from(vocab::UNK) {
                cards[id] = card_row(kind, false);
                upgraded[id] = card_row(kind, true);
            }
        }
        let mut relics = vec![[0; WIDTH]; vocab::RELICS.ids().count()];
        for kind in RelicKind::ALL {
            let id = usize::from(vocab::relic(kind));
            if id > usize::from(vocab::UNK) {
                relics[id] = relic_row(kind);
            }
        }
        let mut potions = vec![[0; WIDTH]; vocab::POTIONS.ids().count()];
        for kind in PotionKind::ALL {
            let id = usize::from(vocab::potion(kind));
            if id > usize::from(vocab::UNK) {
                potions[id] = potion_row(kind);
            }
        }
        [cards, upgraded, relics, potions]
    });
    &all[match table {
        Table::Cards => 0,
        Table::CardsUpgraded => 1,
        Table::Relics => 2,
        Table::Potions => 3,
    }]
}

/// All of [`table`]'s rows as one `i16` slice (row-major), for the FFI.
#[must_use]
pub fn flat(t: Table) -> &'static [i16] {
    table(t).as_flattened()
}

/// A number as the column stores it (truncated, saturated).
fn int(value: Decimal) -> i16 {
    let v = value.to_int_truncating();
    i16::try_from(v).unwrap_or(if v < 0 { i16::MIN } else { i16::MAX })
}

/// Fills the var columns from `(name, value)` in declaration order.
fn put_vars(row: &mut Row, vars: impl IntoIterator<Item = (String, Decimal)>) {
    let mut others = 0;
    let mut count = 0;
    for (name, value) in vars {
        count += 1;
        if let Some(i) = VAR_COLUMNS.iter().position(|&c| c == name) {
            row[NUMBER_AT + 2 + i] = int(value);
        } else if name.ends_with("Power") {
            if row[column("power_other")] == 0 {
                row[column("power_other")] = int(value);
            }
        } else if others < 2 {
            row[column(if others == 0 { "other_a" } else { "other_b" })] = int(value);
            others += 1;
        }
    }
    row[column("vars")] = count;
}

fn set(row: &mut Row, name: &str, on: bool) {
    row[column(name)] = i16::from(on);
}

fn put_target(row: &mut Row, target: TargetType) {
    let name = match target {
        TargetType::Myself => "target_self",
        TargetType::None => "target_none",
        TargetType::AnyEnemy => "target_enemy",
        TargetType::AllEnemies => "target_all_enemies",
        TargetType::RandomEnemy => "target_random_enemy",
        TargetType::AnyPlayer => "target_player",
    };
    set(row, name, true);
}

/// A card's flags and numbers at one upgrade level (no deltas).
fn card_level(kind: CardKind, upgraded: bool) -> Row {
    let mut row = [0; WIDTH];
    let card = Card::new(CardId(0), kind, u8::from(upgraded));
    set(&mut row, "is_card", true);
    set(
        &mut row,
        match kind.card_type() {
            CardType::Attack => "attack",
            CardType::Skill => "skill",
            CardType::Power => "power",
            CardType::Status => "status",
            CardType::Curse => "curse",
            CardType::Quest => "quest",
        },
        true,
    );
    set(
        &mut row,
        match kind.rarity() {
            CardRarity::None => "rarity_none",
            CardRarity::Basic => "basic",
            CardRarity::Common => "common",
            CardRarity::Uncommon => "uncommon",
            CardRarity::Rare => "rare",
            CardRarity::Ancient => "ancient",
            CardRarity::Event => "event",
            CardRarity::Token => "token",
            CardRarity::Status => "rarity_status",
            CardRarity::Curse => "rarity_curse",
            CardRarity::Quest => "rarity_quest",
        },
        true,
    );
    put_target(&mut row, kind.target());
    for (i, keyword) in [
        CardKeyword::Exhaust,
        CardKeyword::Ethereal,
        CardKeyword::Innate,
        CardKeyword::Unplayable,
        CardKeyword::Retain,
        CardKeyword::Sly,
        CardKeyword::Eternal,
    ]
    .into_iter()
    .enumerate()
    {
        row[KEYWORD_AT + i] = i16::from(card.keywords.contains(keyword));
    }
    set(&mut row, "cost_x", card.energy_cost.costs_x);
    set(&mut row, "star_cost_x", card.star_cost.costs_x);
    set(
        &mut row,
        "has_star_cost",
        card.star_cost.costs_x || card.star_cost.base >= 0,
    );
    row[column("cost")] = int(Decimal::from(card.energy_cost.base.max(0)));
    row[column("star_cost")] = int(Decimal::from(card.star_cost.base.max(0)));
    set(&mut row, "osty_attack", kind.is_osty_attack());
    set(&mut row, "strike", kind.is_strike());
    set(&mut row, "defend", kind.is_defend());
    set(&mut row, "shiv", kind.is_shiv());
    set(&mut row, "minion", kind.is_minion());
    set(&mut row, "gains_block", kind.gains_block());
    set(&mut row, "colorless", kind.is_colorless());
    set(
        &mut row,
        "turn_end_in_hand",
        kind.has_turn_end_in_hand_effect(),
    );
    let vars: &[CardVar] = kind.vars();
    for v in vars.iter().filter(|v| v.multiplier.is_some()) {
        let name = match v.name {
            Var::CalculatedDamage => "calculated_damage",
            Var::CalculatedBlock => "calculated_block",
            _ => "calculated_other",
        };
        set(&mut row, name, true);
    }
    // A calculated var's value is its `CalculationBase` (its own column): only its flag.
    put_vars(
        &mut row,
        vars.iter()
            .filter(|v| v.multiplier.is_none())
            .map(|v| (v.name.to_string(), card.var(v.name))),
    );
    set(&mut row, "upgradable", kind.is_upgradable(0));
    set(&mut row, "upgraded", upgraded);
    row
}

/// A card's row: its level's flags and numbers, and the upgrade's deltas (the same on both
/// rows: what the upgrade gives).
fn card_row(kind: CardKind, upgraded: bool) -> Row {
    let up = kind.is_upgradable(0);
    let base = card_level(kind, false);
    let plus = if up { card_level(kind, true) } else { base };
    let mut row = if upgraded && up { plus } else { base };
    if upgraded && !up {
        set(&mut row, "upgraded", false);
    }
    for i in 0..NUMBERS {
        row[DELTA_AT + i] = plus[NUMBER_AT + i].saturating_sub(base[NUMBER_AT + i]);
    }
    for i in 0..KEYWORDS {
        row[DELTA_AT + NUMBERS + i] = plus[KEYWORD_AT + i] - base[KEYWORD_AT + i];
    }
    row
}

fn relic_row(kind: RelicKind) -> Row {
    let mut row = [0; WIDTH];
    set(&mut row, "is_relic", true);
    put_vars(
        &mut row,
        kind.canonical_vars()
            .iter()
            .filter_map(|v| Some((v.name.to_owned(), v.value?))),
    );
    row
}

fn potion_row(kind: PotionKind) -> Row {
    let mut row = [0; WIDTH];
    set(&mut row, "is_potion", true);
    put_target(&mut row, kind.target());
    set(
        &mut row,
        match kind.usage() {
            PotionUsage::CombatOnly => "combat_only",
            PotionUsage::AnyTime => "any_time",
            PotionUsage::Automatic => "automatic",
        },
        true,
    );
    put_vars(
        &mut row,
        kind.vars().into_iter().map(|(n, v)| (n.to_owned(), v)),
    );
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(kind: CardKind, upgraded: bool) -> &'static Row {
        let t = if upgraded {
            Table::CardsUpgraded
        } else {
            Table::Cards
        };
        &table(t)[usize::from(vocab::card(kind))]
    }

    fn get(row: &Row, name: &str) -> i16 {
        row[column(name)]
    }

    #[test]
    fn columns_are_unique_and_as_wide_as_a_row() {
        let cols = columns();
        assert_eq!(cols.len(), WIDTH);
        for (i, c) in cols.iter().enumerate() {
            assert!(
                cols[..i].iter().all(|d| d.name != c.name),
                "{} twice",
                c.name
            );
        }
        assert_eq!(column("exhaust"), KEYWORD_AT);
        assert_eq!(column("cost"), NUMBER_AT);
        assert_eq!(column("Damage"), NUMBER_AT + 2);
        assert_eq!(column("d_cost"), DELTA_AT);
        assert_eq!(column("d_exhaust"), DELTA_AT + NUMBERS);
    }

    /// The rows match the engine's numbers for known cards (the card text in brackets).
    #[test]
    fn known_cards_match_the_engine() {
        // Wrought in War: Attack, common, cost 1, deal 7 (9) damage, Forge 7 (9).
        for up in [false, true] {
            let row = card(CardKind::WroughtInWar, up);
            let c = Card::new(CardId(0), CardKind::WroughtInWar, u8::from(up));
            assert_eq!(i32::from(get(row, "Damage")), c.int_var(Var::Damage));
            assert_eq!(i32::from(get(row, "Forge")), c.int_var(Var::Forge));
            assert_eq!(get(row, "Damage"), if up { 9 } else { 7 });
            assert_eq!(get(row, "attack"), 1);
            assert_eq!(get(row, "common"), 1);
            assert_eq!(get(row, "target_enemy"), 1);
            assert_eq!(get(row, "cost"), 1);
            assert_eq!(get(row, "d_Damage"), 2);
            assert_eq!(get(row, "d_Forge"), 2);
            assert_eq!(get(row, "upgraded"), i16::from(up));
            assert_eq!(get(row, "vars"), 2);
        }
        // Backflip: Skill, 1 energy, 5 (8) block, draw 2.
        let (b, p) = (
            card(CardKind::Backflip, false),
            card(CardKind::Backflip, true),
        );
        assert_eq!((get(b, "Block"), get(p, "Block")), (5, 8));
        assert_eq!((get(b, "Cards"), get(p, "Cards")), (2, 2));
        assert_eq!((get(b, "skill"), get(b, "gains_block")), (1, 1));
        assert_eq!(get(b, "d_Block"), 3);
        // Zap: cost 1 (0 upgraded): the cost delta.
        let (b, p) = (card(CardKind::Zap, false), card(CardKind::Zap, true));
        assert_eq!((get(b, "cost"), get(p, "cost")), (1, 0));
        assert_eq!((get(b, "d_cost"), get(p, "d_cost")), (-1, -1));
        assert_eq!(get(b, "basic"), 1);
        assert_eq!(get(b, "target_self"), 1);
    }

    /// Every ported card's row agrees with a fresh engine card on every plain var with a column,
    /// the cost and the keywords.
    #[test]
    fn every_card_matches_a_fresh_card() {
        for kind in CardKind::ALL {
            for up in [false, true] {
                let level = u8::from(up && kind.is_upgradable(0));
                let c = Card::new(CardId(0), kind, level);
                let row = card(kind, up);
                assert_eq!(get(row, "is_card"), 1, "{kind:?}");
                assert_eq!(
                    i32::from(get(row, "cost")),
                    c.energy_cost.base.max(0),
                    "{kind:?}"
                );
                assert_eq!(
                    get(row, "unplayable") == 1,
                    c.keywords.contains(CardKeyword::Unplayable),
                    "{kind:?}"
                );
                for v in kind.vars().iter().filter(|v| v.multiplier.is_none()) {
                    let name = v.name.to_string();
                    if let Some(i) = VAR_COLUMNS.iter().position(|&n| n == name) {
                        assert_eq!(
                            i32::from(row[NUMBER_AT + 2 + i]),
                            c.var(v.name).to_int_truncating(),
                            "{kind:?} {name}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn unknown_ids_are_zero_and_relics_and_potions_have_rows() {
        for t in [
            Table::Cards,
            Table::CardsUpgraded,
            Table::Relics,
            Table::Potions,
        ] {
            assert!(table(t)[0].iter().all(|&v| v == 0));
            assert!(table(t)[1].iter().all(|&v| v == 0));
        }
        let relics = table(Table::Relics);
        assert!(relics.iter().filter(|r| r[column("is_relic")] == 1).count() > 100);
        let potions = table(Table::Potions);
        assert!(
            potions
                .iter()
                .filter(|r| r[column("is_potion")] == 1)
                .count()
                > 40
        );
    }
}
