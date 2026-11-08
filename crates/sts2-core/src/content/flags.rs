//! The flags the game derives from a class declaration (constructor arguments, overrides,
//! interfaces, card tags and keywords, pool membership), pinned for every ported entity and every
//! hand-written pool table.
//!
//! `flags/<kind>.tsv` holds the game's values for every model class, generated from the decomp
//! by `tools/flag_truth.py` (ids and values only). A newly ported entity is checked as soon as it
//! is registered: copy its flags from the class declaration or this test fails. A `?` cell is a
//! value the generator cannot decide (an override that reads the state) and is not checked.

use std::collections::{BTreeSet, HashMap};
use std::fmt::Display;

use super::afflictions::AfflictionKind;
use super::cards::{CardKeyword, CardKind, CardType, TargetType, pools};
use super::characters::Character;
use super::enchantments::EnchantmentKind;
use super::potions::PotionKind;
use super::powers::PowerKind;
use super::relics::RelicKind;
use super::relics::neows_bones::AVAILABLE_CURSES;
use crate::combat::hooks::PowerRef;
use crate::combat::state::Power;
use crate::combat::{Combat, CombatConfig, CombatState, CreatureId, PowerId};
use crate::run::cards::{COLORLESS_CARD_POOL, ETERNAL_CARDS, EVENT_DECK_CARDS, PoolCard};
use crate::run::potions::{NOT_GENERATED_IN_COMBAT, SHARED_POTION_POOL};
use crate::run::relic_bag::SHARED_RELIC_POOL;
use crate::run::shop::NOT_IN_SHOPS;

const CARD_TYPES: [CardType; 6] = [
    CardType::Attack,
    CardType::Skill,
    CardType::Power,
    CardType::Status,
    CardType::Curse,
    CardType::Quest,
];

/// One `flags/<kind>.tsv`: a header row, then one row per model class keyed by its id.
struct Table {
    name: &'static str,
    columns: Vec<&'static str>,
    rows: HashMap<&'static str, Vec<&'static str>>,
}

impl Table {
    fn parse(name: &'static str, text: &'static str) -> Self {
        let mut lines = text.lines();
        let columns: Vec<_> = lines.next().expect("a header").split('\t').collect();
        let rows = lines
            .map(|line| {
                let cells: Vec<_> = line.split('\t').collect();
                assert_eq!(cells.len(), columns.len(), "{name}: {line}");
                (cells[0], cells)
            })
            .collect();
        Self {
            name,
            columns,
            rows,
        }
    }

    fn cell(&self, id: &str, column: &str) -> &'static str {
        let index = self
            .columns
            .iter()
            .position(|c| *c == column)
            .unwrap_or_else(|| panic!("{}: no column {column}", self.name));
        self.rows[id][index]
    }

    /// The ids whose `column` is `value`.
    fn ids_where(&self, column: &str, value: &str) -> BTreeSet<&'static str> {
        self.rows
            .keys()
            .copied()
            .filter(|id| self.cell(id, column) == value)
            .collect()
    }

    /// The ids in `pool` (a relic can be in several pools: `Event+Shared`).
    fn ids_in_pool(&self, pool: &str) -> BTreeSet<&'static str> {
        self.rows
            .keys()
            .copied()
            .filter(|id| self.cell(id, "pool").split('+').any(|p| p == pool))
            .collect()
    }
}

/// The mismatches found so far, reported together.
#[derive(Default)]
struct Audit(Vec<String>);

impl Audit {
    /// Whether `id` has a row; records a mismatch if not.
    fn has(&mut self, table: &Table, id: &str) -> bool {
        let found = table.rows.contains_key(id);
        if !found {
            self.0
                .push(format!("{id}: not in flags/{}.tsv", table.name));
        }
        found
    }

    fn check(&mut self, table: &Table, id: &str, column: &str, actual: impl Display) {
        let expected = table.cell(id, column);
        let actual = actual.to_string();
        if expected != "?" && expected != actual {
            self.0
                .push(format!("{id}.{column}: game {expected}, ported {actual}"));
        }
    }

    fn same<T: Ord + std::fmt::Debug>(
        &mut self,
        what: &str,
        game: impl IntoIterator<Item = T>,
        ported: impl IntoIterator<Item = T>,
    ) {
        let game: BTreeSet<T> = game.into_iter().collect();
        let ported: BTreeSet<T> = ported.into_iter().collect();
        if game != ported {
            self.0.push(format!(
                "{what}: only in the game {:?}, only ported {:?}",
                game.difference(&ported).collect::<Vec<_>>(),
                ported.difference(&game).collect::<Vec<_>>()
            ));
        }
    }

    fn finish(self) {
        assert!(
            self.0.is_empty(),
            "{} mismatches:\n{}",
            self.0.len(),
            self.0.join("\n")
        );
    }
}

fn cards() -> Table {
    Table::parse("cards", include_str!("flags/cards.tsv"))
}

fn debug(value: impl std::fmt::Debug) -> String {
    format!("{value:?}")
}

fn target(target: TargetType) -> String {
    match target {
        TargetType::Myself => "Self".to_owned(),
        other => debug(other),
    }
}

/// The `CardType`s a `can_*_card_type` accepts, as the table writes them (`*` for all).
fn card_types(accepts: impl Fn(CardType) -> bool) -> String {
    if CARD_TYPES.into_iter().all(&accepts) {
        return "*".to_owned();
    }
    CARD_TYPES
        .into_iter()
        .filter(|&t| accepts(t))
        .map(debug)
        .collect::<Vec<_>>()
        .join("|")
}

#[test]
fn ported_cards_match_the_game() {
    let table = cards();
    let mut audit = Audit::default();
    for kind in CardKind::ALL {
        let id = kind.id();
        if !audit.has(&table, id) {
            continue;
        }
        let model = kind.model();
        audit.check(&table, id, "type", debug(model.card_type()));
        audit.check(&table, id, "rarity", debug(model.rarity()));
        audit.check(&table, id, "target", target(model.target()));
        audit.check(&table, id, "cost_x", model.has_energy_cost_x());
        if !model.has_energy_cost_x() {
            audit.check(&table, id, "cost", model.cost());
        }
        audit.check(&table, id, "star_cost_x", model.has_star_cost_x());
        if !model.has_star_cost_x() {
            audit.check(&table, id, "star_cost", model.star_cost());
        }
        let keywords = model.keywords();
        for (column, keyword) in [
            ("exhaust", CardKeyword::Exhaust),
            ("ethereal", CardKeyword::Ethereal),
            ("innate", CardKeyword::Innate),
            ("retain", CardKeyword::Retain),
            ("unplayable", CardKeyword::Unplayable),
            ("sly", CardKeyword::Sly),
            ("eternal", CardKeyword::Eternal),
        ] {
            audit.check(&table, id, column, keywords.contains(keyword));
        }
        audit.check(&table, id, "gains_block", model.gains_block());
        audit.check(&table, id, "tag_strike", model.is_strike());
        audit.check(&table, id, "tag_defend", model.is_defend());
        audit.check(&table, id, "tag_osty_attack", model.is_osty_attack());
        audit.check(&table, id, "tag_shiv", model.is_shiv());
        audit.check(&table, id, "tag_minion", model.is_minion());
        audit.check(&table, id, "max_upgrade_level", model.max_upgrade_level());
        audit.check(
            &table,
            id,
            "turn_end_in_hand",
            model.has_turn_end_in_hand_effect(),
        );
    }
    audit.finish();
}

/// One pool table entry against its row.
fn check_pool_card(audit: &mut Audit, table: &Table, card: &PoolCard, pool: &str) {
    if audit.has(table, card.id) {
        audit.check(table, card.id, "pool", pool);
        audit.check(table, card.id, "rarity", debug(card.rarity));
        audit.check(table, card.id, "type", debug(card.card_type));
        audit.check(table, card.id, "multiplayer_only", card.multiplayer_only);
    }
}

#[test]
fn card_pool_tables_match_the_game() {
    let table = cards();
    let mut audit = Audit::default();
    for character in Character::ALL {
        let name = character.name();
        let pool = character.card_pool();
        for card in pool {
            check_pool_card(&mut audit, &table, card, name);
        }
        let ids: BTreeSet<_> = pool.iter().map(|c| c.id).collect();
        audit.same(
            &format!("{name} card pool"),
            table.ids_where("pool", name),
            ids.clone(),
        );
        let pool_where = |column: &str, value: &str| -> BTreeSet<_> {
            ids.iter()
                .copied()
                .filter(|id| table.cell(id, column) == value)
                .collect()
        };
        audit.same(
            &format!("{name} ethereal"),
            pool_where("ethereal", "true"),
            character.ethereal_cards().iter().copied(),
        );
        audit.same(
            &format!("{name} not generated in combat"),
            pool_where("generated_in_combat", "false"),
            character.not_generated_cards().iter().copied(),
        );
        let not_x = pool_where("cost_x", "false");
        audit.same(
            &format!("{name} zero cost"),
            pool_where("cost", "0").intersection(&not_x).copied(),
            character.zero_cost_cards().iter().copied(),
        );
    }
    audit.finish();
}

#[test]
fn shared_card_tables_match_the_game() {
    let table = cards();
    let mut audit = Audit::default();
    for card in &COLORLESS_CARD_POOL {
        check_pool_card(&mut audit, &table, card, "Colorless");
    }
    audit.same(
        "colorless card pool",
        table.ids_where("pool", "Colorless"),
        COLORLESS_CARD_POOL.iter().map(|c| c.id),
    );
    let combat_colorless = pools::colorless();
    for card in &combat_colorless {
        if audit.has(&table, card.id) {
            audit.check(&table, card.id, "rarity", debug(card.rarity));
            audit.check(&table, card.id, "type", debug(card.card_type));
            audit.check(&table, card.id, "ethereal", card.ethereal);
            audit.check(
                &table,
                card.id,
                "generated_in_combat",
                card.can_be_generated_in_combat,
            );
        }
    }
    audit.same(
        "colorless card pool without MultiplayerOnly",
        table
            .ids_where("pool", "Colorless")
            .intersection(&table.ids_where("multiplayer_only", "false"))
            .copied(),
        combat_colorless.iter().map(|c| c.id),
    );

    for (id, card_type) in EVENT_DECK_CARDS {
        if audit.has(&table, id) {
            audit.check(&table, id, "type", debug(card_type));
            audit.check(&table, id, "max_upgrade_level", 0);
        }
    }
    audit.same(
        "curses generated by modifiers",
        table
            .ids_in_pool("Curse")
            .intersection(&table.ids_where("generated_by_modifiers", "true"))
            .copied(),
        AVAILABLE_CURSES,
    );
    audit.same(
        "eternal cards",
        table.ids_where("eternal", "true"),
        ETERNAL_CARDS,
    );
    for &id in table.rows.keys() {
        let colorless = matches!(
            table.cell(id, "visual_pool"),
            "Colorless" | "Token" | "Event" | "Deprecated"
        );
        audit.check(
            &table,
            id,
            "visual_pool",
            if pools::is_visual_pool_colorless(id) == colorless {
                table.cell(id, "visual_pool")
            } else {
                "the other side of IsColorless"
            },
        );
    }
    audit.finish();
}

/// `ShouldOwnerDeathTriggerFatal` of a fresh `kind` power on `enemy`.
fn owner_death_triggers_fatal(s: &mut CombatState, enemy: CreatureId, kind: PowerKind) -> bool {
    let me = PowerRef {
        owner: enemy,
        id: PowerId(u32::MAX),
    };
    s.push_power(
        enemy,
        Power {
            id: me.id,
            kind,
            amount: 1,
            amount_on_turn_start: 1,
            applier: None,
            skip_next_duration_tick: false,
        },
    );
    s.set_power_data(me.id, kind.init_internal_data());
    let fatal = kind.should_owner_death_trigger_fatal(s, me);
    let last = s.creature(enemy).powers.len() - 1;
    s.take_power(enemy, last);
    s.set_power_data(me.id, crate::content::powers::PowerData::None);
    fatal
}

#[test]
fn ported_powers_match_the_game() {
    let table = Table::parse("powers", include_str!("flags/powers.tsv"));
    let mut audit = Audit::default();
    // `ShouldOwnerDeathTriggerFatal` may read the state: evaluate it for the power on an enemy
    // (an override that depends on more than that is a `?` cell).
    let mut state = Combat::new(CombatConfig::necrobinder("94LYYA8YDC", 0))
        .state()
        .clone();
    let enemy = state.enemies[0];
    for kind in PowerKind::ALL {
        let id = kind.id();
        if !audit.has(&table, id) {
            continue;
        }
        audit.check(&table, id, "type", debug(kind.power_type()));
        audit.check(&table, id, "instance_type", debug(kind.instance_type()));
        audit.check(&table, id, "allow_negative", kind.allows_negative());
        audit.check(&table, id, "visible", kind.is_visible());
        audit.check(&table, id, "temporary", kind.is_temporary());
        audit.check(
            &table,
            id,
            "removed_after_owner_death",
            kind.should_be_removed_after_owner_death(),
        );
        audit.check(
            &table,
            id,
            "owner_death_triggers_fatal",
            owner_death_triggers_fatal(&mut state, enemy, kind),
        );
        audit.check(
            &table,
            id,
            "owner_is_secondary_enemy",
            kind.owner_is_secondary_enemy(),
        );
        // `type_for_amount` assumes `StackType` Counter, which only matters with AllowNegative.
        if kind.allows_negative() {
            audit.check(&table, id, "stack_type", "Counter");
        }
    }
    audit.finish();
}

#[test]
fn relic_tables_match_the_game() {
    let table = Table::parse("relics", include_str!("flags/relics.tsv"));
    let mut audit = Audit::default();
    for kind in RelicKind::ALL {
        audit.has(&table, kind.id());
    }
    let pools = Character::ALL
        .into_iter()
        .map(|c| (c.name(), c.relic_pool()))
        .chain([("Shared", &SHARED_RELIC_POOL[..])]);
    for (name, pool) in pools {
        for &(id, rarity) in pool {
            if audit.has(&table, id) {
                audit.check(&table, id, "rarity", debug(rarity));
            }
        }
        audit.same(
            &format!("{name} relic pool"),
            table.ids_in_pool(name),
            pool.iter().map(|&(id, _)| id),
        );
    }
    audit.same(
        "relics not allowed in shops",
        table.ids_where("allowed_in_shops", "false"),
        NOT_IN_SHOPS,
    );
    audit.finish();
}

#[test]
fn potions_match_the_game() {
    let table = Table::parse("potions", include_str!("flags/potions.tsv"));
    let mut audit = Audit::default();
    for kind in PotionKind::ALL {
        let id = kind.id();
        if audit.has(&table, id) {
            audit.check(&table, id, "usage", debug(kind.model().usage()));
            audit.check(&table, id, "target", target(kind.model().target()));
        }
    }
    let pools = Character::ALL
        .into_iter()
        .map(|c| (c.name(), c.potion_pool()))
        .chain([("Shared", &SHARED_POTION_POOL[..])]);
    for (name, pool) in pools {
        for &(id, rarity) in pool {
            if audit.has(&table, id) {
                audit.check(&table, id, "rarity", debug(rarity));
                audit.check(&table, id, "pool", name);
            }
        }
        audit.same(
            &format!("{name} potion pool"),
            table.ids_where("pool", name),
            pool.iter().map(|&(id, _)| id),
        );
    }
    audit.same(
        "potions not generated in combat",
        table.ids_where("generated_in_combat", "false"),
        NOT_GENERATED_IN_COMBAT,
    );
    audit.finish();
}

#[test]
fn ported_enchantments_and_afflictions_match_the_game() {
    let table = Table::parse("enchantments", include_str!("flags/enchantments.tsv"));
    let mut audit = Audit::default();
    for kind in EnchantmentKind::ALL {
        let id = kind.id();
        if audit.has(&table, id) {
            let model = kind.model();
            audit.check(&table, id, "stackable", model.is_stackable());
            audit.check(
                &table,
                id,
                "card_types",
                card_types(|t| model.can_enchant_card_type(t)),
            );
            audit.check(
                &table,
                id,
                "start_at_bottom",
                model.should_start_at_bottom_of_draw_pile(),
            );
        }
    }
    let table = Table::parse("afflictions", include_str!("flags/afflictions.tsv"));
    for kind in AfflictionKind::ALL {
        let id = kind.id();
        if audit.has(&table, id) {
            let model = kind.model();
            audit.check(&table, id, "stackable", model.is_stackable());
            audit.check(
                &table,
                id,
                "card_types",
                card_types(|t| model.can_afflict_card_type(t)),
            );
            audit.check(
                &table,
                id,
                "afflict_unplayable",
                model.can_afflict_unplayable_cards(),
            );
        }
    }
    audit.finish();
}
