//! The per-instance fields of cards, powers, relics and enchantments in the referee's schema.
//!
//! `ModelFields` in `referee/tracer/TraceDump.cs` (XT3) lists the model subclass's own fields
//! (and a power's `_internalData`) as `[name, value]` pairs sorted by name, the C# field name in
//! `snake_case`. A card is its id, a creature a `CreatureRef` (`player`, `pet:<id>`,
//! `enemy:<index in CombatState.Enemies>`), a collection its count, any other object `true` when
//! set. The changed `DynamicVars` of powers and relics are listed the same way (`vars`).
//!
//! A model the game gives fields the engine does not keep has no entry here; the comparison
//! then reports the missing `fields`.

use serde_json::{Value, json};

use sts2_core::combat::{CombatState, CreatureId};
use sts2_core::content::cards::{CardData, CardKind};
use sts2_core::content::enchantments::{Enchantment, EnchantmentKind, EnchantmentStatus};
use sts2_core::content::powers::PowerData;
use sts2_core::content::relics::{RelicData, RelicKind};

use crate::ids;
use crate::session::decimal_json;

/// `[name, value]` pairs sorted by name; `None` when the model has no fields.
fn sorted(mut fields: Vec<(&'static str, Value)>) -> Option<Value> {
    if fields.is_empty() {
        return None;
    }
    fields.sort_by(|a, b| a.0.cmp(b.0));
    Some(Value::Array(
        fields
            .into_iter()
            .map(|(name, value)| json!([name, value]))
            .collect(),
    ))
}

/// A creature as the referee's `CreatureRef`.
#[must_use]
pub fn creature_ref(s: &CombatState, id: Option<CreatureId>) -> Value {
    let Some(id) = id else {
        return Value::Null;
    };
    let c = s.creature(id);
    if c.is_player() {
        return json!("player");
    }
    if c.pet_owner.is_some() {
        let monster = c.monster.as_ref().map(|m| m.kind.id());
        return json!(format!("pet:{}", monster.unwrap_or("?")));
    }
    let index = s
        .enemies
        .iter()
        .position(|&e| e == id)
        .map_or(-1, |i| i64::try_from(i).unwrap_or(-1));
    json!(format!("enemy:{index}"))
}

/// A combat card's or deck card's fields. `data` is the combat card's; a deck card passes the
/// fields its saved counter gives (`cards::data_from_deck`).
#[must_use]
pub fn card(kind: CardKind, data: &CardData) -> Option<Value> {
    let fields = match (kind, data) {
        (CardKind::TheScythe, CardData::TheScythe(d)) => vec![
            ("current_damage", json!(d.current_damage)),
            ("increased_damage", json!(d.increased_damage)),
        ],
        (CardKind::TheScythe, _) => vec![
            ("current_damage", json!(13)),
            ("increased_damage", json!(0)),
        ],
        (CardKind::GeneticAlgorithm, data) => {
            let increased = match data {
                CardData::GeneticAlgorithm(d) => d.increased_block,
                _ => 0,
            };
            vec![
                ("current_block", json!(1 + increased)),
                ("increased_block", json!(increased)),
            ]
        }
        (CardKind::SovereignBlade, data) => {
            let (damage, repeats) = match data {
                CardData::SovereignBlade(d) => {
                    (decimal_json(d.damage.get()), decimal_json(d.repeats.get()))
                }
                _ => (json!(10), json!(1)),
            };
            vec![("current_damage", damage), ("current_repeats", repeats)]
        }
        (CardKind::Rampage, data) => vec![(
            "extra_damage_from_plays",
            json!(match data {
                CardData::Rampage(d) => d.extra_damage_from_plays,
                _ => 0,
            }),
        )],
        (CardKind::Thrash, data) => vec![(
            "extra_damage",
            match data {
                CardData::Thrash(d) => decimal_json(d.extra_damage.get()),
                _ => json!(0),
            },
        )],
        (CardKind::KinglyPunch, data) => vec![(
            "extra_damage",
            json!(match data {
                CardData::KinglyPunch(d) => d.extra_damage,
                _ => 0,
            }),
        )],
        (CardKind::Claw | CardKind::Maul, data) => return claw_or_maul(kind, data),
        (CardKind::UpMySleeve, data) => vec![(
            "times_played_this_combat",
            json!(match data {
                CardData::UpMySleeve(n) => *n,
                _ => 0,
            }),
        )],
        (CardKind::Regret, data) => vec![(
            "cards_in_hand",
            json!(match data {
                CardData::Regret(n) => *n,
                _ => 0,
            }),
        )],
        (CardKind::Guilty | CardKind::Dowsing, data) => {
            let name = if kind == CardKind::Guilty {
                "combats_seen"
            } else {
                "rooms_entered"
            };
            vec![(
                name,
                json!(match data {
                    CardData::Counter(n) => *n,
                    _ => 0,
                }),
            )]
        }
        (CardKind::Wither, data) => vec![(
            "fake_upgrade_level",
            json!(match data {
                CardData::Wither(d) => d.fake_upgrade_level,
                _ => 0,
            }),
        )],
        (CardKind::MadScience, data) => return mad_science(data),
        // `AfterCreated` sets `SpoilsActIndex` to 1; `SpoilsCoord` is set (the deck card's counter)
        // once Act 2's map is generated with the card in the deck.
        (CardKind::SpoilsMap, data) => return spoils_map(matches!(data, CardData::Counter(_))),
        _ => Vec::new(),
    };
    sorted(fields)
}

/// A `Claw`'s or `Maul`'s extra damage from earlier plays of its kind (0 on a fresh card).
fn claw_or_maul(kind: CardKind, data: &CardData) -> Option<Value> {
    let field = match (kind, data) {
        (CardKind::Maul, CardData::Maul(d)) => (
            "extra_damage_from_maul_plays",
            d.extra_damage_from_maul_plays,
        ),
        (CardKind::Maul, _) => ("extra_damage_from_maul_plays", 0),
        (_, CardData::Claw(d)) => (
            "extra_damage_from_claw_plays",
            d.extra_damage_from_claw_plays,
        ),
        _ => ("extra_damage_from_claw_plays", 0),
    };
    sorted(vec![(field.0, json!(field.1))])
}

/// A `MadScience`'s fields: `TinkerTimeType` and `TinkerTimeRider` by enum name (a fresh card's
/// are `None`; `_mockedChaosCard` is test-only).
fn mad_science(data: &CardData) -> Option<Value> {
    let (ty, rider) = match data {
        CardData::MadScience(d) => (
            d.tinker_time_type
                .map_or_else(|| "None".to_owned(), |t| format!("{t:?}")),
            format!("{:?}", d.tinker_time_rider),
        ),
        _ => ("None".to_owned(), "None".to_owned()),
    };
    sorted(vec![
        ("tinker_time_rider", json!(rider)),
        ("tinker_time_type", json!(ty)),
    ])
}

/// A `SpoilsMap`'s fields: `SpoilsCoord` is set (`true`) once `ModifyGeneratedMapLate` noted Act 2's
/// treasure point.
fn spoils_map(coord_set: bool) -> Option<Value> {
    sorted(vec![
        ("spoils_act_index", json!(1)),
        (
            "spoils_coord",
            if coord_set { json!(true) } else { Value::Null },
        ),
    ])
}

/// An enchantment's fields.
#[must_use]
pub fn enchantment(e: &Enchantment) -> Option<Value> {
    match e.kind {
        EnchantmentKind::Momentum => sorted(vec![("extra_damage", json!(e.extra_damage))]),
        // `Glam` sets `UsedThisCombat` and disables itself together, on its first play.
        EnchantmentKind::Glam => sorted(vec![(
            "used_this_combat",
            json!(e.status == EnchantmentStatus::Disabled),
        )]),
        _ => None,
    }
}

/// An object reference the referee shows only as set (`true`) or `null`.
const fn set(is_set: bool) -> Value {
    if is_set {
        Value::Bool(true)
    } else {
        Value::Null
    }
}

/// The number of occupied slots of a played-card table (a C# `Dictionary<CardModel, int>`).
fn count<T>(slots: &[Option<T>]) -> usize {
    slots.iter().filter(|s| s.is_some()).count()
}

/// `HellraiserPower.Data`'s `autoPlayingCards` (a set: its count), `infiniteAutoPlaysThisTurn`
/// and `showedCapReachedMessage`.
fn hellraiser(cards: u32, plays: i32, shown: bool) -> Vec<(&'static str, Value)> {
    vec![
        ("auto_playing_cards", json!(cards)),
        ("infinite_auto_plays_this_turn", json!(plays)),
        ("showed_cap_reached_message", json!(shown)),
    ]
}

/// A power's fields: its own and its `_internalData`'s.
#[must_use]
#[allow(clippy::too_many_lines, reason = "one arm per power data variant")]
pub fn power(s: &CombatState, data: &PowerData) -> Option<Value> {
    let card_id = |c: Option<sts2_core::combat::CardId>| {
        c.map_or(Value::Null, |c| json!(ids::card(s.card(c).kind)))
    };
    let fields = match data {
        PowerData::None
        | PowerData::Slow(_)
        | PowerData::ToricToughness(_)
        | PowerData::TheBomb(_)
        | PowerData::Inferno(_)
        | PowerData::CrimsonMantle(_)
        | PowerData::Thievery(_)
        | PowerData::Heist(_)
        | PowerData::WitheringPresence(_) => Vec::new(),
        PowerData::Illusion(d) => vec![("is_reviving", json!(d.is_reviving))],
        PowerData::Reattach(d) => vec![("is_reviving", json!(d.is_reviving))],
        PowerData::Swipe(d) => vec![("stolen_card", card_id(d.stolen))],
        PowerData::Vigor(d) => vec![
            (
                "amount_when_attack_started",
                json!(d.amount_when_attack_started),
            ),
            ("command_to_modify", set(d.command_to_modify.is_some())),
        ],
        PowerData::Gigantification(d) => {
            vec![("command_to_modify", set(d.command_to_modify.is_some()))]
        }
        PowerData::Oblivion(d) => vec![(
            "amounts_for_played_cards",
            json!(count(&d.amounts_for_played_cards)),
        )],
        PowerData::Strangle(d) => vec![(
            "amounts_for_played_cards",
            json!(count(&d.amounts_for_played_cards)),
        )],
        PowerData::Subroutine(d) => vec![(
            "amounts_for_played_cards",
            json!(count(&d.amounts_for_played_cards)),
        )],
        PowerData::SerpentForm(d) => vec![(
            "amounts_for_played_cards",
            json!(count(&d.amounts_for_played_cards)),
        )],
        PowerData::Storm(d) => vec![(
            "amounts_for_played_cards",
            json!(count(&d.amounts_for_played_cards)),
        )],
        PowerData::Monologue(d) => {
            vec![("amounts_for_played_cards", json!(count(&d.played)))]
        }
        PowerData::Afterimage(d) => vec![("amounts_for_played_cards", json!(d.len))],
        PowerData::Calamity(d) => vec![("amounts_for_played_cards", json!(count(&d.played_cards)))],
        PowerData::Rupture(d) => vec![("played_cards", json!(count(&d.played_cards)))],
        PowerData::Automation(d) => vec![("cards_left", json!(d.cards_left))],
        PowerData::Panache(d) => vec![("already_applied", json!(d.already_applied))],
        PowerData::Ritual(d) => {
            vec![(
                "was_just_applied_by_enemy",
                json!(d.was_just_applied_by_enemy),
            )]
        }
        PowerData::DarkEmbrace(d) => vec![("ethereal_count", json!(d.ethereal_count))],
        PowerData::VoidForm(d) => {
            vec![("cards_played_this_turn", json!(d.cards_played_this_turn))]
        }
        PowerData::Juggling(d) => {
            vec![(
                "attacks_played_this_turn",
                json!(d.attacks_played_this_turn),
            )]
        }
        PowerData::Feral(d) => {
            vec![(
                "zero_cost_attacks_played",
                json!(d.zero_cost_attacks_played),
            )]
        }
        PowerData::CurlUp(d) => vec![("played_card", card_id(d.played_card))],
        PowerData::PossessStrength(d) => {
            vec![("stolen_strength", json!(usize::from(d.stolen.is_some())))]
        }
        PowerData::PossessSpeed(d) => {
            vec![("stolen_dexterity", json!(usize::from(d.stolen.is_some())))]
        }
        PowerData::Nightmare(d) => vec![("selected_card", card_id(d.selected_card))],
        PowerData::Orbit(d) => vec![
            ("energy_spent", json!(d.energy_spent)),
            ("trigger_count", json!(d.trigger_count)),
        ],
        PowerData::PaleBlueDot(d) => vec![(
            "already_activated_this_turn",
            json!(d.already_activated_this_turn),
        )],
        PowerData::HardenedShell(d) => vec![(
            "damage_received_this_turn",
            json!(d.damage_received_this_turn),
        )],
        PowerData::Tender(d) => vec![("cards_played_this_turn", json!(d.cards_played_this_turn))],
        PowerData::Dampen(d) => vec![
            ("casters", json!(d.casters_count())),
            (
                "downgraded_cards_to_old_upgrade_levels",
                json!(d.downgraded_count()),
            ),
        ],
        PowerData::Hellraiser(d) => hellraiser(
            d.auto_playing_cards,
            d.infinite_auto_plays_this_turn,
            d.showed_cap_reached_message,
        ),
        PowerData::Sloth(d) => vec![("cards_played_this_turn", json!(d.cards_played_this_turn))],
        PowerData::Surrounded(d) => vec![("facing", json!(format!("{:?}", d.facing)))],
        // `_initialAmount` only feeds creature positions and `AfterApplied` skips it in test mode
        // (the referee's), so it stays 0 there; the engine does not keep it.
        PowerData::Sandpit(_) => vec![("initial_amount", json!(0))],
        PowerData::Skittish(d) => vec![(
            "has_gained_block_this_turn",
            json!(d.has_gained_block_this_turn),
        )],
        PowerData::ChainsOfBinding(d) => vec![("bound_card_played", json!(d.bound_card_played))],
        PowerData::Adaptable(d) => vec![("is_reviving", json!(d.is_reviving))],
        PowerData::Nemesis(d) => {
            vec![("should_apply_intangible", json!(d.should_apply_intangible))]
        }
    };
    sorted(fields)
}

/// A power's `DynamicVars` that differ from the canonical instance's, `[name, value]` in
/// declaration order (the referee's power `vars`). Only the vars the engine keeps state in.
#[must_use]
pub fn power_vars(data: &PowerData) -> Option<Value> {
    let vars: Vec<Value> = match data {
        PowerData::Slow(d) => {
            let amount = d.slow_amount;
            if amount == 0 {
                Vec::new()
            } else {
                vec![
                    json!(["SlowAmount", amount]),
                    json!(["DisplayAmount", amount * 10]),
                ]
            }
        }
        PowerData::ToricToughness(d) => changed("Block", decimal_json(d.block), &json!(0)),
        PowerData::TheBomb(d) => changed("Damage", decimal_json(d.damage), &json!(40)),
        // `PowerVar<StrengthPower>` (canonical 1) and `StrengthApplied` (0).
        PowerData::Monologue(d) => {
            let mut vars = changed("StrengthPower", json!(d.strength), &json!(1));
            vars.extend(changed(
                "StrengthApplied",
                json!(d.strength_applied),
                &json!(0),
            ));
            vars
        }
        PowerData::Panache(d) => changed("CardsLeft", json!(d.cards_left), &json!(5)),
        PowerData::Inferno(d) => changed("SelfDamage", json!(d.self_damage), &json!(0)),
        PowerData::CrimsonMantle(d) => changed("SelfDamage", json!(d.self_damage), &json!(0)),
        PowerData::Thievery(d) => changed("Gold", json!(d.gold), &json!(0)),
        // `CardsLeft` (canonical 6).
        PowerData::WitheringPresence(d) => changed("CardsLeft", json!(d.cards_left), &json!(6)),
        _ => Vec::new(),
    };
    (!vars.is_empty()).then_some(Value::Array(vars))
}

/// `[name, value]` unless `value` is the canonical one.
fn changed(name: &str, value: Value, canonical: &Value) -> Vec<Value> {
    if value == *canonical {
        Vec::new()
    } else {
        vec![Value::Array(vec![json!(name), value])]
    }
}

/// A power's `Target` (the engine keeps one only for the powers that use it).
#[must_use]
pub fn power_target(s: &CombatState, data: &PowerData) -> Value {
    match data {
        PowerData::Thievery(d) => creature_ref(s, d.target),
        PowerData::Heist(d) => creature_ref(s, d.target),
        PowerData::Swipe(d) => creature_ref(s, d.target),
        PowerData::Sandpit(d) => creature_ref(s, d.target),
        PowerData::WitheringPresence(d) => creature_ref(s, d.target),
        _ => Value::Null,
    }
}

/// A relic's fields. `s` resolves card ids (a relic only holds one during a fight).
#[must_use]
#[allow(clippy::too_many_lines, reason = "one arm per relic data variant")]
pub fn relic(s: Option<&CombatState>, data: &RelicData) -> Option<Value> {
    let card_id = |c: Option<sts2_core::combat::CardId>| match (c, s) {
        (Some(c), Some(s)) => json!(ids::card(s.card(c).kind)),
        (Some(_), None) => json!(true),
        (None, _) => Value::Null,
    };
    let fields = match data {
        RelicData::None | RelicData::EmotionChip(_) => Vec::new(),
        RelicData::Nunchaku(d) => vec![("attacks_played", json!(d.attacks_played))],
        RelicData::PaelsTears(d) => vec![("had_leftover_energy", json!(d.had_leftover_energy))],
        RelicData::PumpkinCandle(d) => vec![("kindle_count", json!(d.kindle_count))],
        RelicData::GoldenCompass(d) => vec![("golden_path_act", json!(d.golden_path_act))],
        RelicData::PaelsEye(d) => vec![
            ("used_this_combat", json!(d.used_this_combat)),
            (
                "was_owner_part_of_last_player_turn",
                json!(d.was_owner_part_of_last_player_turn),
            ),
        ],
        // `_affectedCardPlay` is an object: whether it is set (`ModelFields.Value`).
        RelicData::PaelsLegion(d) => vec![
            (
                "affected_card_play",
                json!(d.affected_card_play.map(|_| true)),
            ),
            ("cooldown", json!(d.cooldown)),
            (
                "triggered_block_last_turn",
                json!(d.triggered_block_last_turn),
            ),
        ],
        // `_serializableCards` as its count; `_skin` is a string (skipped).
        RelicData::PaelsTooth(d) => vec![("serializable_cards", json!(d.len()))],
        RelicData::FishingRod(d) => vec![("combats_seen", json!(d.combats_seen))],
        RelicData::LavaRock(d) => vec![("has_triggered", json!(d.has_triggered))],
        RelicData::SilverCrucible(d) => vec![
            ("times_used", json!(d.times_used)),
            ("treasure_rooms_entered", json!(d.treasure_rooms_entered)),
        ],
        RelicData::WingedBoots(d) => vec![("times_used", json!(d.times_used))],
        RelicData::BoneTea(d) => vec![("combats_left", json!(d.combats_left))],
        RelicData::EmberTea(d) => vec![("combats_left", json!(d.combats_left))],
        RelicData::TeaOfDiscourtesy(d) => vec![("combats_left", json!(d.combats_left))],
        RelicData::SilkenTress(d) => vec![("is_used", json!(d.is_used))],
        RelicData::Girya(d) => vec![("times_lifted", json!(d.times_lifted))],
        RelicData::PaelsWing(d) => vec![("rewards_sacrificed", json!(d.rewards_sacrificed))],
        RelicData::ToyBox(d) => vec![("combats_seen", json!(d.combats_seen))],
        RelicData::LavaLamp(d) => {
            vec![("took_damage_this_combat", json!(d.took_damage_this_combat))]
        }
        RelicData::LastingCandy(d) => {
            vec![("combat_rewards_seen", json!(d.combat_rewards_seen))]
        }
        RelicData::SwordOfStone(d) => vec![("elites_defeated", json!(d.elites_defeated))],
        RelicData::VenerableTeaSet(d) => vec![(
            "gain_energy_in_next_combat",
            json!(d.gain_energy_in_next_combat),
        )],
        RelicData::ArtOfWar(d) => vec![
            (
                "any_attacks_played_last_turn",
                json!(d.any_attacks_played_last_turn),
            ),
            (
                "any_attacks_played_this_turn",
                json!(d.any_attacks_played_this_turn),
            ),
        ],
        RelicData::BeatingRemnant(d) => vec![(
            "damage_received_this_turn",
            json!(d.damage_received_this_turn),
        )],
        RelicData::BeltBuckle(d) => vec![("dexterity_applied", json!(d.dexterity_applied))],
        RelicData::BookOfFiveRings(d) => vec![("cards_added", json!(d.cards_added))],
        RelicData::BurningSticks(d) => {
            vec![("was_used_this_combat", json!(d.was_used_this_combat))]
        }
        RelicData::CentennialPuzzle(d) => {
            vec![("used_this_combat", json!(d.used_this_combat))]
        }
        RelicData::HappyFlower(d) => vec![("turns_seen", json!(d.turns_seen))],
        RelicData::JossPaper(d) => vec![
            ("cards_exhausted", json!(d.cards_exhausted)),
            ("ethereal_count", json!(d.ethereal_count)),
        ],
        RelicData::Kunai(d) => {
            vec![(
                "attacks_played_this_turn",
                json!(d.attacks_played_this_turn),
            )]
        }
        RelicData::Kusarigama(d) => {
            vec![(
                "attacks_played_this_turn",
                json!(d.attacks_played_this_turn),
            )]
        }
        RelicData::LetterOpener(d) => {
            vec![("skills_played_this_turn", json!(d.skills_played_this_turn))]
        }
        RelicData::LizardTail(d) => vec![("was_used", json!(d.was_used))],
        RelicData::Orichalcum(d) => vec![("should_trigger", json!(d.should_trigger))],
        RelicData::OrnamentalFan(d) => {
            vec![(
                "attacks_played_this_turn",
                json!(d.attacks_played_this_turn),
            )]
        }
        RelicData::PenNib(d) => vec![
            ("attack_to_double", card_id(d.attack_to_double)),
            ("attacks_played", json!(d.attacks_played)),
        ],
        RelicData::Pendulum(d) => vec![("turns_seen", json!(d.turns_seen))],
        RelicData::PollinousCore(d) => vec![("turns_seen", json!(d.turns_seen))],
        RelicData::BingBong(d) => vec![("cards_to_skip", json!(d.cards_to_skip))],
        RelicData::WongosMysteryTicket(d) => vec![
            ("combats_finished", json!(d.combats_finished)),
            ("gave_relic", json!(d.gave_relic)),
        ],
        RelicData::Permafrost(d) => {
            vec![("activated_this_combat", json!(d.activated_this_combat))]
        }
        RelicData::Pocketwatch(d) => vec![
            ("cards_played_last_turn", json!(d.cards_played_last_turn)),
            ("cards_played_this_turn", json!(d.cards_played_this_turn)),
        ],
        RelicData::RainbowRing(d) => vec![
            ("activation_count_this_turn", json!(d.activations)),
            ("attacks_played_this_turn", json!(d.attacks)),
            ("powers_played_this_turn", json!(d.powers)),
            ("skills_played_this_turn", json!(d.skills)),
        ],
        RelicData::Shuriken(d) => {
            vec![(
                "attacks_played_this_turn",
                json!(d.attacks_played_this_turn),
            )]
        }
        RelicData::TuningFork(d) => vec![("skills_played", json!(d.skills_played))],
        RelicData::UnsettlingLamp(d) => vec![
            // `DoubledPowers` gets the triggering power (any) once the triggering card is set; the
            // engine keeps only its temporary stat.
            (
                "doubled_powers",
                json!(usize::from(d.triggering_card.is_some())),
            ),
            ("is_finished_triggering", json!(d.is_finished_triggering)),
            ("triggering_card", card_id(d.triggering_card)),
        ],
        RelicData::Vambrace(d) => vec![
            (
                "block_gained_this_combat",
                json!(d.block_gained_this_combat),
            ),
            ("triggering_card", card_id(d.triggering_card)),
        ],
        RelicData::DemonTongue(d) => vec![("triggered_this_turn", json!(d.triggered_this_turn))],
        RelicData::RedSkull(d) => vec![("strength_applied", json!(d.strength_applied))],
        RelicData::RuinedHelmet(d) => vec![("used_this_combat", json!(d.used_this_combat))],
        RelicData::Metronome(d) => vec![("orbs_channeled", json!(d.orbs_channeled))],
        RelicData::GalacticDust(d) => vec![("stars_spent", json!(d.stars_spent))],
        RelicData::MiniRegent(d) => vec![("used_this_turn", json!(d.used_this_turn))],
        RelicData::Regalite(d) => vec![("used_this_turn", json!(d.used_this_turn))],
        RelicData::MawBank(d) => vec![("has_item_been_bought", json!(d.has_item_been_bought))],
        RelicData::VelvetChoker(d) => {
            vec![("cards_played_this_turn", json!(d.cards_played_this_turn))]
        }
        RelicData::DustyTome(d) => vec![("ancient_card", json!(d.ancient_card))],
        RelicData::MusicBox(d) => vec![
            ("was_used_this_turn", json!(d.was_used_this_turn)),
            ("card_being_played", card_id(d.card_being_played)),
        ],
        // `ModelId.Entry` of the character (`NECROBINDER`).
        RelicData::SeaGlass(d) => vec![(
            "character_id",
            json!(d.character.map(|c| c.name().to_ascii_uppercase())),
        )],
        RelicData::TouchOfOrobas(d) => vec![
            ("starter_relic", json!(d.starter_relic)),
            ("upgraded_relic", json!(d.upgraded_relic)),
        ],
        // A `SerializableCard` is traced as `true`.
        RelicData::IronClub(d) => vec![("cards_played", json!(d.cards_played))],
        RelicData::ThrowingAxe(d) => vec![("used_this_combat", json!(d.used_this_combat))],
        RelicData::ArchaicTooth(d) => vec![
            (
                "serializable_ancient_card",
                json!(d.ancient_card.map(|_| true)),
            ),
            (
                "serializable_starter_card",
                json!(d.starter_card.map(|_| true)),
            ),
        ],
        RelicData::BrilliantScarf(d) => {
            vec![("cards_played_this_turn", json!(d.cards_played_this_turn))]
        }
        RelicData::FurCoat(d) => vec![
            ("fur_coat_act_index", json!(d.fur_coat_act_index)),
            ("fur_coat_coord_cols", json!(d.len())),
            ("fur_coat_coord_rows", json!(d.len())),
            ("fur_coat_coords_set", json!(d.coords_set)),
        ],
    };
    sorted(fields)
}

/// A relic's `DynamicVars` that differ from the canonical instance's (the referee's relic
/// `vars`): only the vars the engine keeps state in.
#[must_use]
pub fn relic_vars(data: &RelicData) -> Option<Value> {
    let vars = match data {
        // `CombatsLeft` mirrored into `Combats` (canonical 5 and 1).
        RelicData::EmberTea(d) => changed("Combats", json!(d.combats_left), &json!(5)),
        RelicData::BoneTea(d) => changed("Combats", json!(d.combats_left), &json!(1)),
        RelicData::TeaOfDiscourtesy(d) => changed("Combats", json!(d.combats_left), &json!(1)),
        // `Rooms` is `3 - TimesUsed`.
        RelicData::WingedBoots(d) => changed("Rooms", json!(3 - d.times_used), &json!(3)),
        // `RemainingCombats` is `max(5 - CombatsFinished, 0)`, set in `AfterCombatEnd`.
        RelicData::WongosMysteryTicket(d) => changed(
            "RemainingCombats",
            json!((5 - d.combats_finished).max(0)),
            &json!(5),
        ),
        _ => Vec::new(),
    };
    (!vars.is_empty()).then_some(Value::Array(vars))
}

/// A relic in the referee's `{id, vars, fields}` schema.
#[must_use]
pub fn relic_json(s: Option<&CombatState>, kind: RelicKind, data: &RelicData) -> Value {
    let mut json = json!({"id": kind.id()});
    if let Some(vars) = relic_vars(data) {
        json["vars"] = vars;
    }
    if let Some(fields) = relic(s, data) {
        json["fields"] = fields;
    }
    json
}
