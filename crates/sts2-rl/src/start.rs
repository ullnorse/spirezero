//! Episode starts and the start corpus (`docs/specs/rl-2x-plan.md`, section 7.5).
//!
//! A [`StartRecord`] is a [`CombatConfig`]-shaped record (character, seed string, ascension,
//! act, encounter, deck with upgrades, relics with their fields, belt, HP, gold, floor, the
//! run's RNG streams) kept as one JSON object per line. `cargo xtask sweep --policy greedy --emit-starts FILE` writes one for every Act 1
//! combat its runs enter ([`StartRecord::from_run`]). A record is built into a [`Combat`] once
//! ([`StartRecord::combat`], `Combat::new` plus `start_combat`) and an env resets from that
//! with `clone_from`.
//!
//! The relics' fields and the streams are in their core text form ([`sts2_core::text`]):
//! `relic_data` holds each relic's [`RelicData`] and `IsMelted`, `rng` the run's
//! [`RunRngSet`] as the fight's set-up received it. Records written before they existed (RC1)
//! have neither and still load: their relics start with their initial data and their streams
//! are fresh ones from the seed string.

use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::{Value, json};
use sts2_core::combat::{BeltPotion, CombatConfig, DeckCard, Encounter};
use sts2_core::content::cards::CardKind;
use sts2_core::content::enchantments::{Enchantment, EnchantmentKind, EnchantmentStatus};
use sts2_core::content::potions::PotionKind;
use sts2_core::content::relics::{OwnedRelic, RelicData, RelicKind};
use sts2_core::rng::{RunRngSet, canonicalize_seed};
use sts2_core::run::Run;
use sts2_core::run::rooms::OVERGROWTH;
use sts2_core::text::{from_text, to_text};
use sts2_core::{Character, Combat, Status};

/// A deck card of a record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StartCard {
    pub kind: CardKind,
    pub upgrade_level: u8,
    pub enchantment: Option<Enchantment>,
    pub counter: u32,
}

/// One episode start: what [`CombatConfig`] needs, in a form that survives a text file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartRecord {
    pub character: Character,
    /// The combat's RNG seed string (canonicalized like a run seed).
    pub seed: String,
    pub ascension: u8,
    pub act_index: i32,
    pub encounter: Encounter,
    pub deck: Vec<StartCard>,
    pub relics: Vec<RelicKind>,
    pub potion_slots: usize,
    /// The belt by slot: a potion id (ported or not) or an empty slot.
    pub potions: Vec<Option<String>>,
    pub hp: i32,
    pub max_hp: i32,
    pub max_energy: i32,
    pub gold: i32,
    pub total_floor: i32,
    pub map_coord: Option<(i32, i32)>,
    /// Each relic's fields and `IsMelted`, by relic; `None` for fresh relics.
    pub relic_data: Option<Vec<(RelicData, bool)>>,
    /// The run's streams as of combat start; `None` for fresh streams from `seed`.
    pub rng: Option<RunRngSet>,
}

/// An unported potion id as the `'static` string [`BeltPotion::Unported`] holds (interned, so
/// loading a corpus twice leaks nothing new).
fn intern(id: &str) -> &'static str {
    static IDS: Mutex<Option<HashMap<String, &'static str>>> = Mutex::new(None);
    let mut ids = IDS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    ids.get_or_insert_with(HashMap::new)
        .entry(id.to_owned())
        .or_insert_with(|| Box::leak(id.to_owned().into_boxed_str()))
}

/// A JSON field of `object`.
fn field<'a>(object: &'a Value, name: &str) -> Result<&'a Value, String> {
    object
        .get(name)
        .ok_or_else(|| format!("missing field {name}"))
}

/// An integer field of `object`.
fn int<T: TryFrom<i64>>(object: &Value, name: &str) -> Result<T, String> {
    let value = field(object, name)?
        .as_i64()
        .ok_or_else(|| format!("{name} is not an integer"))?;
    T::try_from(value).map_err(|_| format!("{name} is out of range"))
}

/// A string field of `object`.
fn text<'a>(object: &'a Value, name: &str) -> Result<&'a str, String> {
    field(object, name)?
        .as_str()
        .ok_or_else(|| format!("{name} is not a string"))
}

/// An array field of `object`.
fn array<'a>(object: &'a Value, name: &str) -> Result<&'a [Value], String> {
    field(object, name)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("{name} is not an array"))
}

/// An `i64` element of a JSON array.
fn element(values: &[Value], i: usize) -> Result<i64, String> {
    values
        .get(i)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("element {i} is not an integer"))
}

/// Checks that `data` has one entry per relic of `relics`, each of the relic's own kind.
fn check_relic_data(relics: &[RelicKind], data: &[(RelicData, bool)]) -> Result<(), String> {
    if relics.len() != data.len() {
        return Err(format!(
            "{} relics but {} relic data",
            relics.len(),
            data.len()
        ));
    }
    for (kind, (data, _)) in relics.iter().zip(data) {
        if std::mem::discriminant(&kind.init_data()) != std::mem::discriminant(data) {
            return Err(format!("{} has the data of another relic", kind.id()));
        }
    }
    Ok(())
}

impl StartCard {
    fn to_json(self) -> Value {
        let enchantment = self.enchantment.map_or(Value::Null, |e| {
            json!([
                e.kind.id(),
                e.amount,
                e.status == EnchantmentStatus::Disabled,
                e.extra_damage
            ])
        });
        json!([
            self.kind.id(),
            self.upgrade_level,
            enchantment,
            self.counter
        ])
    }

    fn from_json(value: &Value) -> Result<Self, String> {
        let parts = value.as_array().ok_or("a deck card is not an array")?;
        let id = parts.first().and_then(Value::as_str).ok_or("card id")?;
        let kind = CardKind::from_id(id).ok_or_else(|| format!("unknown card {id}"))?;
        let int = |i| -> Result<i64, String> { element(parts, i) };
        let enchantment = match parts.get(2) {
            None | Some(Value::Null) => None,
            Some(Value::Array(e)) => {
                let id = e.first().and_then(Value::as_str).ok_or("enchantment id")?;
                let kind = EnchantmentKind::from_id(id)
                    .ok_or_else(|| format!("unknown enchantment {id}"))?;
                let status = if e.get(2).and_then(Value::as_bool).unwrap_or(false) {
                    EnchantmentStatus::Disabled
                } else {
                    EnchantmentStatus::Normal
                };
                let small = |i| -> Result<i32, String> {
                    i32::try_from(element(e, i)?).map_err(|_| "enchantment field".to_owned())
                };
                Some(Enchantment {
                    kind,
                    amount: small(1)?,
                    status,
                    extra_damage: small(3)?,
                })
            }
            Some(_) => return Err("an enchantment is not an array".into()),
        };
        Ok(Self {
            kind,
            upgrade_level: u8::try_from(int(1)?).map_err(|_| "upgrade level")?,
            enchantment,
            counter: u32::try_from(int(3)?).map_err(|_| "counter")?,
        })
    }
}

impl StartRecord {
    /// The record of the combat `run` is in, under the seed string `seed`; `None` outside a
    /// combat. The streams are recorded if the run keeps them
    /// ([`Run::with_combat_start_streams`]); the relics' fields always are (the run writes a
    /// fight's back only when it ends).
    #[must_use]
    pub fn from_run(run: &Run, seed: String) -> Option<Self> {
        let combat = run.combat()?;
        let cs = combat.state();
        let rs = run.state();
        Some(Self {
            character: rs.character,
            seed,
            ascension: rs.ascension,
            act_index: cs.act_index,
            encounter: cs.encounter,
            deck: rs
                .deck
                .iter()
                .filter_map(|c| c.to_combat())
                .map(|c| StartCard {
                    kind: c.kind,
                    upgrade_level: c.upgrade_level,
                    enchantment: c.enchantment,
                    counter: c.counter,
                })
                .collect(),
            relics: rs.relics.iter().map(|r| r.kind).collect(),
            potion_slots: rs.potions.len(),
            potions: rs.potions.iter().map(|p| p.map(str::to_owned)).collect(),
            hp: rs.hp,
            max_hp: rs.max_hp,
            max_energy: rs.max_energy,
            gold: rs.gold,
            total_floor: i32::try_from(rs.total_floor()).unwrap_or(i32::MAX),
            map_coord: cs.map_coord,
            relic_data: Some(rs.relics.iter().map(|r| (r.data, r.melted)).collect()),
            rng: run.combat_start_streams().cloned(),
        })
    }

    /// The record as one line of JSON.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut record = json!({
            "character": self.character.name(),
            "seed": self.seed,
            "ascension": self.ascension,
            "act": self.act_index,
            "encounter": self.encounter.entry(),
            "deck": self.deck.iter().map(|c| c.to_json()).collect::<Vec<_>>(),
            "relics": self.relics.iter().map(|r| r.id()).collect::<Vec<_>>(),
            "potion_slots": self.potion_slots,
            "potions": self.potions,
            "hp": self.hp,
            "max_hp": self.max_hp,
            "max_energy": self.max_energy,
            "gold": self.gold,
            "floor": self.total_floor,
            "map_coord": self.map_coord.map(<[i32; 2]>::from),
        });
        // Left out when absent, so an older record writes back as it was.
        if let Some(data) = &self.relic_data {
            record["relic_data"] = data.iter().map(to_text).collect();
        }
        if let Some(rng) = &self.rng {
            record["rng"] = to_text(rng).into();
        }
        record.to_string()
    }

    /// A record from one line of JSON.
    ///
    /// # Errors
    /// If the line is not a record or names content the engine does not have.
    pub fn from_json(line: &str) -> Result<Self, String> {
        let v: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        let name = text(&v, "character")?;
        let character =
            Character::from_name(name).ok_or_else(|| format!("unknown character {name}"))?;
        let id = text(&v, "encounter")?;
        let encounter =
            Encounter::from_entry(id).ok_or_else(|| format!("unknown encounter {id}"))?;
        let relics = array(&v, "relics")?
            .iter()
            .map(|r| {
                let id = r.as_str().ok_or("a relic is not a string")?;
                RelicKind::from_id(id).ok_or_else(|| format!("unknown relic {id}"))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let potions = array(&v, "potions")?
            .iter()
            .map(|p| match p {
                Value::Null => Ok(None),
                Value::String(id) => Ok(Some(id.clone())),
                _ => Err("a potion is not a string or null".to_owned()),
            })
            .collect::<Result<_, String>>()?;
        let relic_data = match v.get("relic_data") {
            None => None,
            Some(data) => {
                let data = data
                    .as_array()
                    .ok_or("relic_data is not an array")?
                    .iter()
                    .map(|d| {
                        let text = d.as_str().ok_or("a relic's data is not a string")?;
                        from_text::<(RelicData, bool)>(text).map_err(|e| e.to_string())
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                check_relic_data(&relics, &data)?;
                Some(data)
            }
        };
        let rng = match v.get("rng") {
            None => None,
            Some(rng) => Some(
                from_text::<RunRngSet>(rng.as_str().ok_or("rng is not a string")?)
                    .map_err(|e| format!("rng: {e}"))?,
            ),
        };
        let map_coord = match field(&v, "map_coord")? {
            Value::Null => None,
            Value::Array(c) => Some((
                i32::try_from(element(c, 0)?).map_err(|_| "map column")?,
                i32::try_from(element(c, 1)?).map_err(|_| "map row")?,
            )),
            _ => return Err("map_coord is not an array or null".into()),
        };
        Ok(Self {
            character,
            seed: text(&v, "seed")?.to_owned(),
            ascension: int(&v, "ascension")?,
            act_index: int(&v, "act")?,
            encounter,
            deck: array(&v, "deck")?
                .iter()
                .map(StartCard::from_json)
                .collect::<Result<_, _>>()?,
            relics,
            potion_slots: int(&v, "potion_slots")?,
            potions,
            hp: int(&v, "hp")?,
            max_hp: int(&v, "max_hp")?,
            max_energy: int(&v, "max_energy")?,
            gold: int(&v, "gold")?,
            total_floor: int(&v, "floor")?,
            map_coord,
            relic_data,
            rng,
        })
    }

    /// The combat set-up of the record.
    #[must_use]
    pub fn config(&self) -> CombatConfig {
        let config = CombatConfig {
            character: self.character,
            rng: self
                .rng
                .clone()
                .unwrap_or_else(|| RunRngSet::new(&canonicalize_seed(&self.seed))),
            ascension: self.ascension,
            encounter: self.encounter,
            room_type: sts2_core::run::RoomType::Monster,
            deck: self
                .deck
                .iter()
                .map(|c| DeckCard {
                    kind: c.kind,
                    upgrade_level: c.upgrade_level,
                    enchantment: c.enchantment,
                    counter: c.counter,
                })
                .collect(),
            relics: self.relic_data.as_ref().map_or_else(
                || self.relics.iter().map(|&r| OwnedRelic::from(r)).collect(),
                |data| {
                    self.relics
                        .iter()
                        .zip(data)
                        .map(|(&kind, &(data, melted))| OwnedRelic { kind, data, melted })
                        .collect()
                },
            ),
            potion_slots: self.potion_slots,
            potions: self
                .potions
                .iter()
                .map(|p| {
                    p.as_deref().map(|id| {
                        PotionKind::from_id(id)
                            .map_or_else(|| BeltPotion::Unported(intern(id)), BeltPotion::Ported)
                    })
                })
                .collect(),
            player_hp: self.hp,
            player_max_hp: self.max_hp,
            max_energy: self.max_energy,
            gold: self.gold,
            total_floor: self.total_floor,
            act_index: self.act_index,
            map_coord: self.map_coord,
            record_events: false,
            modifiers: sts2_core::run::modifiers::CombatModifiers::default(),
        };
        // The room type of the encounter's list (Monster, Elite or Boss).
        config.with_encounter(self.encounter)
    }

    /// The record's combat at its first decision point; `None` if it stops at unported content
    /// or is already over.
    #[must_use]
    pub fn combat(&self) -> Option<Combat> {
        let combat = Combat::new(self.config());
        matches!(combat.status(), Status::PlayerTurn | Status::Decision).then_some(combat)
    }
}

/// One start: a label seed (the policy and the bench digest use it) and the combat an episode
/// resets from.
#[derive(Clone, Debug)]
pub struct Start {
    pub seed: u64,
    pub combat: Combat,
}

impl Start {
    /// The RL bench's start for `seed` (`sts2-bench`'s `fight_start`, W1-all): character
    /// `seed % 5`'s starter deck on Overgrowth encounter `seed % 22`, seed string `W1S{seed}`.
    ///
    /// # Panics
    /// On a 32-bit target, for a seed above `u32::MAX`.
    #[must_use]
    pub fn w1_all(seed: u64) -> Self {
        let n = usize::try_from(seed).expect("a 64-bit usize");
        let character = Character::ALL[n % Character::ALL.len()];
        let info = &OVERGROWTH.encounters[n % OVERGROWTH.encounters.len()];
        let encounter =
            Encounter::from_entry(info.id).expect("every Overgrowth encounter is ported");
        let config = CombatConfig::new(character, &format!("W1S{seed}"), 0)
            .expect("every starter card is ported")
            .with_encounter(encounter);
        Self {
            seed,
            combat: Combat::new(config),
        }
    }
}

/// The order episodes take the starts in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// Each start `reps` times in turn, then no more episodes: episode `e` is start `e / reps`,
    /// repetition `e % reps`.
    Sequential { reps: u64 },
    /// Forever, round-robin: episode `e` is start `e % len`, repetition `e / len`.
    Cycle,
    /// Forever, each episode's start drawn from the env batch's seed and the episode number.
    Random,
}

/// An episode: its number in the source's order, its start and repetition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Episode {
    pub serial: u64,
    pub start: usize,
    pub rep: u64,
}

/// Where episodes start from.
#[derive(Clone, Debug)]
pub struct StartSource {
    starts: Vec<Start>,
    order: Order,
}

/// `SplitMix64` of `x`.
const fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

impl StartSource {
    /// A source over `starts`.
    #[must_use]
    pub const fn new(starts: Vec<Start>, order: Order) -> Self {
        Self { starts, order }
    }

    /// A source over the records that build a playable combat ([`StartRecord::combat`]), each
    /// labelled with its index; the records that do not are skipped.
    #[must_use]
    pub fn from_records(records: &[StartRecord], order: Order) -> Self {
        let starts = records
            .iter()
            .filter_map(StartRecord::combat)
            .zip(0..)
            .map(|(combat, seed)| Start { seed, combat })
            .collect();
        Self::new(starts, order)
    }

    /// [`StartSource::from_records`] over a JSON-lines corpus (blank lines skipped).
    ///
    /// # Errors
    /// If a line is not a record (with its line number).
    pub fn parse(corpus: &str, order: Order) -> Result<Self, String> {
        let records = corpus
            .lines()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty())
            .map(|(i, line)| {
                StartRecord::from_json(line).map_err(|e| format!("line {}: {e}", i + 1))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::from_records(&records, order))
    }

    /// The starts.
    #[must_use]
    pub fn starts(&self) -> &[Start] {
        &self.starts
    }

    /// Episode `serial` under `seed`, if the order has one.
    #[must_use]
    pub fn episode(&self, serial: u64, seed: u64) -> Option<Episode> {
        let len = u64::try_from(self.starts.len()).ok()?;
        if len == 0 {
            return None;
        }
        let (start, rep) = match self.order {
            Order::Sequential { reps } => {
                if serial >= len.checked_mul(reps)? {
                    return None;
                }
                (serial / reps, serial % reps)
            }
            Order::Cycle => (serial % len, serial / len),
            Order::Random => (mix(seed ^ mix(serial)) % len, serial),
        };
        Some(Episode {
            serial,
            start: usize::try_from(start).ok()?,
            rep,
        })
    }
}
