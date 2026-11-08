//! Entity hit counts: which entity ids a generated run or fight touched, for `tools/hits.sh`.
//!
//! The ids are the referee's (`BODYGUARD`, `DOOM_POWER`, `SLIMES_WEAK`). A hit is any appearance: a
//! card in a pile, the deck or the loadout, a power on a creature, an enemy, an owned relic or
//! potion, an enchantment or affliction on a card, an encounter fought, an event entered.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use sts2_core::combat::{Combat, Event};
use sts2_core::run::state::{MapPointHistoryEntry, RoomType};

use crate::Result;

/// Entity ids seen so far, by kind (`card`, `relic`, `potion`, `power`, `monster`, `encounter`,
/// `event`, `enchantment`, `affliction`).
#[derive(Debug, Default)]
pub struct Hits {
    ids: BTreeMap<&'static str, BTreeSet<String>>,
    /// Entities whose effect actually ran in a combat, not merely appeared: a played card, a used
    /// potion, a power whose amount changed. `tools/hits.sh --effects` reports the gap.
    effects: BTreeMap<&'static str, BTreeSet<String>>,
    /// The character played and, for a whole run, the Act 1 (`tools/hits.sh` counts hits per character).
    scope: Option<(&'static str, Option<&'static str>)>,
    /// The player lost HP in a combat step of the trace (see [`Hits::implied`]).
    hp_lost: bool,
}

/// Arrays of cards, by key (combat piles and the run deck).
const CARD_ARRAYS: &[&str] = &["hand", "draw", "discard", "exhaust", "play", "deck"];

impl Hits {
    /// Records the character (`Ironclad`) and the Act 1 key (`overgrowth`, none for a forced fight)
    /// that the written file names.
    pub const fn set_scope(&mut self, character: &'static str, act: Option<&'static str>) {
        self.scope = Some((character, act));
    }

    fn add(&mut self, kind: &'static str, id: &str) {
        self.ids.entry(kind).or_default().insert(id.to_owned());
    }

    fn add_effect(&mut self, kind: &'static str, id: &str) {
        self.effects.entry(kind).or_default().insert(id.to_owned());
    }

    /// The effects that ran so far in `combat`'s event log (or in a fight that has just ended).
    /// Card plays, potion uses and power changes are observable events; a relic's presence is a
    /// hit, but its trigger is not attributable here yet.
    pub fn observe_combat(&mut self, combat: Option<&Combat>) {
        let Some(combat) = combat else {
            return;
        };
        let state = combat.state();
        for event in combat.events() {
            match event {
                Event::CardPlayed { card, .. } => {
                    let kind = state.card(*card).kind;
                    let id = kind.id();
                    self.add_effect("card", id);
                }
                Event::PotionUsed { potion, .. } => self.add_effect("potion", potion.id()),
                Event::PowerChanged { kind, .. } => self.add_effect("power", kind.id()),
                _ => {}
            }
        }
    }

    /// Everything in one trace line (`gen` or `run-gen` format).
    pub fn observe_line(&mut self, line: &Value) {
        if let Some(encounter) = line.pointer("/action/encounter").and_then(Value::as_str) {
            self.add("encounter", encounter);
        }
        self.walk(line);
    }

    /// Everything in a whole trace.
    pub fn observe_trace(&mut self, trace: &[Value]) {
        let hp = |line: &Value| line.pointer("/state/player/hp").and_then(Value::as_i64);
        for (before, line) in trace.iter().zip(trace.iter().skip(1)) {
            let over = before
                .pointer("/state/combat_over")
                .and_then(Value::as_bool);
            if over == Some(false) && hp(line) < hp(before) && hp(line).is_some() {
                self.hp_lost = true;
            }
        }
        for line in trace {
            self.observe_line(line);
        }
    }

    fn walk(&mut self, value: &Value) {
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    self.walk_key(key, child);
                    self.walk(child);
                }
            }
            Value::Array(items) => items.iter().for_each(|item| self.walk(item)),
            _ => {}
        }
    }

    fn walk_key(&mut self, key: &str, child: &Value) {
        // Owned relics and potions are plain strings; objects under those keys are shop and
        // reward offers.
        let owned = matches!(key, "relics" | "potions");
        let ids = |items: &Value| -> Vec<String> {
            items
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|item| match item {
                    Value::String(id) => Some(id.as_str()),
                    _ if owned => None,
                    _ => item.get("id").and_then(Value::as_str),
                })
                .map(str::to_owned)
                .collect()
        };
        let kind = match key {
            k if CARD_ARRAYS.contains(&k) => "card",
            "powers" => "power",
            "enemies" => "monster",
            "relics" => "relic",
            "potions" => "potion",
            _ => "",
        };
        if !kind.is_empty() {
            for id in ids(child) {
                self.add(kind, &id);
            }
        }
        match key {
            // The player's Osty is not an `enemies` entry; the trace reports it under `osty`.
            "osty" if child.get("alive").and_then(Value::as_bool) == Some(true) => {
                self.add("monster", "OSTY");
            }
            "enchantment" | "affliction" => {
                let kind = if key == "enchantment" {
                    "enchantment"
                } else {
                    "affliction"
                };
                if let Some(id) = child.get("id").or(Some(child)).and_then(Value::as_str) {
                    self.add(kind, id);
                }
            }
            "event" => {
                if let Some(id) = child.as_str() {
                    self.add("event", id);
                }
            }
            _ => {}
        }
    }

    /// The loadout's deck, relics and potions (ids may carry `+N` and `:ENCHANTMENT*N` suffixes).
    pub fn observe_loadout(&mut self, loadout: &Value) {
        for (field, kind) in [("deck", "card"), ("relics", "relic"), ("potions", "potion")] {
            for id in loadout
                .get(field)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if let Some(id) = id.as_str() {
                    let mut parts = id.splitn(2, ':');
                    let card = parts.next().unwrap_or_default();
                    self.add(kind, card.split('+').next().unwrap_or(card));
                    if let Some(enchantment) = parts.next() {
                        self.add(
                            "enchantment",
                            enchantment.split('*').next().unwrap_or(enchantment),
                        );
                    }
                }
            }
        }
    }

    /// The encounters fought and events entered, from the run's map-point history.
    pub fn observe_history(&mut self, history: &[MapPointHistoryEntry]) {
        for entry in history {
            let kind = match entry.room_type {
                Some(RoomType::Monster | RoomType::Elite | RoomType::Boss) => "encounter",
                Some(RoomType::Event) => "event",
                _ => continue,
            };
            if let Some(model) = entry.model {
                self.add(kind, model);
            }
        }
    }

    /// Writes `{"card": [...], ...}` to `path`, with the hits the trace cannot show (see
    /// [`Hits::implied`]).
    ///
    /// # Errors
    /// If the file cannot be written.
    pub fn write(&self, path: &str) -> Result<()> {
        let mut doc = json!(self.implied());
        if let Some((character, act)) = self.scope {
            doc["character"] = json!(character);
            if let Some(act) = act {
                doc["act"] = json!(act);
            }
        }
        doc["effects"] = json!(self.effects);
        std::fs::write(path, doc.to_string() + "\n")?;
        Ok(())
    }

    /// The hits plus what they imply, because the trace does not dump it: the cards' afflictions
    /// (`TANGLED_POWER` afflicts every card with `ENTANGLED`, `RINGING_POWER` with `RINGING`,
    /// `SMOGGY_POWER`, which only `LivingFog` puts on the player, with `SMOG`), the `BYRDPIP` pet
    /// (a monster on the player's side that the `BYRDPIP` relic adds each fight) and the
    /// `SELF_FORMING_CLAY_POWER` that the relic applies on HP loss and that is gone again when
    /// the next turn begins, before any line shows it.
    fn implied(&self) -> BTreeMap<&'static str, BTreeSet<String>> {
        let mut hits = self.ids.clone();
        let has = |kind: &str, id: &str| self.ids.get(kind).is_some_and(|ids| ids.contains(id));
        let mut implied = Vec::new();
        for (power, affliction) in [
            ("TANGLED_POWER", "ENTANGLED"),
            ("RINGING_POWER", "RINGING"),
            ("SMOGGY_POWER", "SMOG"),
        ] {
            if has("power", power) {
                implied.push(("affliction", affliction));
            }
        }
        if has("relic", "BYRDPIP") && self.ids.contains_key("encounter") {
            implied.push(("monster", "BYRDPIP"));
        }
        if has("relic", "SELF_FORMING_CLAY") && self.hp_lost {
            implied.push(("power", "SELF_FORMING_CLAY_POWER"));
        }
        for (kind, id) in implied {
            hits.entry(kind).or_default().insert(id.to_owned());
        }
        hits
    }
}
