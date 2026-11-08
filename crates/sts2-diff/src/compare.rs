//! Finding the first disagreement between the engine's trace and the referee's.
//!
//! The engine's trace is the schema: only fields it emits are compared, so referee fields the
//! engine does not model (gold, stars, ...) are ignored without a list. Everything else must
//! match, and a trace the comparison cannot line up is a divergence, never a skip: a referee
//! `error` line, a referee trace shorter or longer than the engine's, an empty engine trace, and
//! in run mode an engine line without a fight where the referee dumps one.
//!
//! # Leniency rules
//!
//! These are every place a field is not compared one-for-one; each has a test below.
//!
//! 1. Referee keys the engine does not emit are not compared (the engine's trace is the schema),
//!    except the [`OPTIONAL_KEYS`] of a model object, which the engine's side gets as `null`.
//!    (`referee_only_keys_are_ignored`, `optional_keys_the_engine_lacks_diverge`)
//! 2. A card's `vars` are not an optional key: an unported card lists none on the engine's side,
//!    so they are compared only when the engine lists them. (`unported_card_without_vars_is_not_compared`)
//! 3. The [`RELATIVE_STREAM`] count is compared as draws since step 0 in a forced fight.
//!    (`up_front_is_compared_from_step_0`)
//! 4. The referee's `move_log` (the full list) is compared in the engine's summary form.
//!    (`referee_move_log_is_summarized`)
//! 5. Older referee builds: a key a referee build does not emit at all is dropped from the
//!    engine's side ([`Schema`]). This is decided once per trace, from every line of it: if any
//!    object of the trace has the marker key, every object must have it, so a key missing from
//!    one object diverges instead of switching the comparison off.
//!    (`older_referee_traces_drop_what_they_lack`, `a_marker_missing_from_one_object_diverges`)
//! 6. After a won fight, an empty slot of the engine's potion belt takes the referee's potion
//!    (sts2-cli claims the fight's potion reward on its own). (`won_fight_belt_takes_the_reward_potion`)
//! 7. Enemy intents are not compared once the player is dead or the fight is over (the engine
//!    finds a move's attacks by playing it), nor for a dead enemy (the game shows none).
//!    (`intents_after_the_fight_and_of_dead_enemies_are_not_compared`)
//! 8. Run mode: an engine line without a fight (`state` `{}`) where the referee still dumps the
//!    fight that ended (a card reward rerolled by `Driftwood`'s `card_alternative`: the engine
//!    writes the ended fight only on the line it ends) is accepted only if the engine's previous
//!    line showed that fight over and the referee's dump is its previous line's apart from `rng`
//!    (which `run.rng` compares). (`run_mode_compares_an_empty_engine_state`)

use std::collections::BTreeSet;
use std::fmt::{self, Write as _};

use serde_json::Value;

/// The first point where two traces differ.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Divergence {
    /// The trace step (0 is the start of the fight).
    pub step: usize,
    /// The field path inside the step, e.g. `state.enemies[0].hp`.
    pub path: String,
    pub rust: Value,
    pub referee: Value,
}

impl fmt::Display for Divergence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "step {} {}: rust {} vs referee {}",
            self.step, self.path, self.rust, self.referee
        )
    }
}

impl Divergence {
    /// The path with list indices removed (`state.enemies[].hp`), to group equal causes.
    #[must_use]
    pub fn shape(&self) -> String {
        let mut out = String::new();
        let mut skipping = false;
        for c in self.path.chars() {
            match c {
                '[' => {
                    skipping = true;
                    out.push('[');
                }
                ']' => {
                    skipping = false;
                    out.push(']');
                }
                _ if skipping => {}
                _ => out.push(c),
            }
        }
        out
    }
}

/// The map and room generation of the referee's `start_run` draws from the `UpFront` stream
/// before the fight; the engine's single combat does not. That stream is compared as the number
/// of draws since step 0, every other stream by its absolute count (leniency rule 3).
const RELATIVE_STREAM: &str = "UpFront";

/// `state` with the relative stream's count taken from `base_state`'s (the trace's step 0).
fn rebased(state: &Value, base_state: &Value) -> Value {
    let mut state = state.clone();
    let counter = |s: &Value| s["rng"]["run"][RELATIVE_STREAM].as_i64();
    if let (Some(base), Some(count)) = (counter(base_state), counter(&state)) {
        state["rng"]["run"][RELATIVE_STREAM] = Value::from(count - base);
    }
    state
}

/// A difference inside one trace line: the path, the engine's value and the referee's.
type Difference = (String, Value, Value);

/// Walks the two traces line by line and returns the first divergence: a referee `error` line,
/// a line one side lacks, an empty engine trace, or the first difference `compare` finds in a pair
/// of lines.
fn walk(
    rust: &[Value],
    referee: &[Value],
    mut compare: impl FnMut(usize, &Value, &Value) -> Option<Difference>,
) -> Option<Divergence> {
    if rust.is_empty() {
        return Some(Divergence {
            step: 0,
            path: "trace.len".to_owned(),
            rust: Value::from(0),
            referee: Value::from(referee.len()),
        });
    }
    for (step, rust_line) in rust.iter().enumerate() {
        let divergence = |(path, rust, referee): Difference| Divergence {
            step,
            path,
            rust,
            referee,
        };
        let Some(referee_line) = referee.get(step) else {
            return Some(divergence((
                "step".to_owned(),
                Value::from(step),
                Value::Null,
            )));
        };
        if let Some(error) = referee_line.get("error") {
            return Some(divergence(("error".to_owned(), Value::Null, error.clone())));
        }
        if let Some(difference) = compare(step, rust_line, referee_line) {
            return Some(divergence(difference));
        }
    }
    (referee.len() > rust.len()).then(|| Divergence {
        step: rust.len(),
        path: "step".to_owned(),
        rust: Value::Null,
        referee: Value::from(rust.len()),
    })
}

/// The first divergence of a forced fight's `referee` trace from `rust`'s, both lists of trace lines.
#[must_use]
pub fn first_divergence(rust: &[Value], referee: &[Value]) -> Option<Divergence> {
    let schema = Schema::of(referee);
    let bases = rust.first().zip(referee.first());
    walk(rust, referee, |_, rust_line, referee_line| {
        let (rust_state, referee_state) = match bases {
            Some((rust0, referee0)) => (
                rebased(&rust_line["state"], &rust0["state"]),
                rebased(&referee_line["state"], &referee0["state"]),
            ),
            None => (rust_line["state"].clone(), referee_line["state"].clone()),
        };
        state_diff(&rust_state, &referee_state, &schema)
            .map(|(path, rust, referee)| (format!("state{path}"), rust, referee))
    })
}

/// Keys a model object (a card, enchantment, power or relic: an object with an `id`) lists only
/// when it has them. The engine's trace is otherwise the schema, so one the referee lists and the
/// engine does not is compared as missing (`null`), not skipped (XT3).
const OPTIONAL_KEYS: [&str; 4] = ["enchantment", "affliction", "vars", "fields"];

/// The keys newer referee builds added to an object kind, each with the key that marks a trace
/// whose build has them; a trace without the marker anywhere drops the keys on the engine's side
/// (leniency rule 5). XT3, and `relic_state` for the run object.
const MARKED_KEYS: [(&str, &[&str]); 6] = [
    // A card in a pile, in `pending` or the deck.
    (
        "star_cost",
        &["star_cost", "last_stars", "clone", "affliction", "fields"],
    ),
    ("enchantment_amount", &["enchantment_amount", "fields"]),
    // A power.
    (
        "skip_tick",
        &[
            "amount_on_turn_start",
            "skip_tick",
            "applier",
            "target",
            "vars",
            "fields",
        ],
    ),
    // An enemy.
    (
        "move_log",
        &[
            "spawned_this_turn",
            "performed_first_move",
            "move_log",
            "intents",
        ],
    ),
    // The combat player.
    (
        "max_energy",
        &["max_energy", "turn_number", "pets", "relics"],
    ),
    // The run object.
    ("relic_state", &["relic_state", "removals_used"]),
];

/// A part of the trace schema that older referee builds lack.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Part {
    /// A [`MARKED_KEYS`] marker on some object.
    Marker(&'static str),
    /// `vars` on a combat card (T15; with them came `replay`).
    CombatCardVars,
    /// `vars` on a card of a `run` object (C23).
    RunCardVars,
    /// The combat player's `potions` (T10).
    Potions,
    /// The combat player's `orbs`.
    Orbs,
}

/// What a referee trace's build emits, sniffed once over the whole trace (leniency rule 5): a
/// part counts as present if any line has it, and is then compared everywhere.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Schema(BTreeSet<Part>);

impl Schema {
    fn of(referee: &[Value]) -> Self {
        let mut schema = Self::default();
        for line in referee {
            schema.note_markers(line);
            let state = &line["state"];
            let player = &state["player"];
            let parts = [
                (Part::Potions, player.get("potions").is_some()),
                (Part::Orbs, player.get("orbs").is_some()),
                (
                    Part::CombatCardVars,
                    PILES.iter().any(|pile| {
                        state[pile]
                            .as_array()
                            .is_some_and(|cards| cards.iter().any(|c| c.get("vars").is_some()))
                    }),
                ),
                (Part::RunCardVars, has_card_with_vars(&line["run"])),
            ];
            schema.0.extend(
                parts
                    .into_iter()
                    .filter(|&(_, has)| has)
                    .map(|(part, _)| part),
            );
        }
        schema
    }

    fn has(&self, part: Part) -> bool {
        self.0.contains(&part)
    }

    fn note_markers(&mut self, value: &Value) {
        match value {
            Value::Object(o) => {
                for (marker, _) in MARKED_KEYS {
                    if o.contains_key(marker) {
                        self.0.insert(Part::Marker(marker));
                    }
                }
                o.values().for_each(|v| self.note_markers(v));
            }
            Value::Array(a) => a.iter().for_each(|v| self.note_markers(v)),
            _ => {}
        }
    }
}

/// A card object of a `run` (an `id` and an `upgrade`).
fn is_card(o: &serde_json::Map<String, Value>) -> bool {
    o.contains_key("id") && o.contains_key("upgrade")
}

fn has_card_with_vars(value: &Value) -> bool {
    match value {
        Value::Object(o) => {
            (is_card(o) && o.contains_key("vars")) || o.values().any(has_card_with_vars)
        }
        Value::Array(a) => a.iter().any(has_card_with_vars),
        _ => false,
    }
}

/// Drops `vars` from every card of the engine's `run` object (a referee build before C23).
fn strip_card_vars(value: &mut Value) {
    match value {
        Value::Object(o) => {
            if is_card(o) {
                o.remove("vars");
            }
            o.values_mut().for_each(strip_card_vars);
        }
        Value::Array(a) => a.iter_mut().for_each(strip_card_vars),
        _ => {}
    }
}

/// The engine keeps an enemy's `StateLog` as its length, last entries and set of states
/// (`MoveLog`); this is the referee's full log of `ids`, oldest first, in that form
/// (`session::move_log_json`; leniency rule 4). Traces are compared at every step, so a
/// divergent move still shows in the step it happens.
fn move_log_summary(ids: &[Value]) -> Value {
    let mut used: Vec<&Value> = ids.iter().collect();
    used.sort_by_key(|v| v.as_str().unwrap_or_default());
    used.dedup();
    serde_json::json!({
        "len": ids.len(),
        "recent": ids.iter().rev().take(3).collect::<Vec<_>>(),
        "used": used,
    })
}

/// Reconciles `rust` with `referee` (both walked together, `rust` on its own keys): drops the
/// [`MARKED_KEYS`] whose marker the referee's trace lacks everywhere, and makes an
/// [`OPTIONAL_KEYS`] key the referee lists and the engine does not a `null` on the engine's side.
fn reconcile(rust: &mut Value, referee: &mut Value, schema: &Schema) {
    match (rust, referee) {
        (Value::Object(r), Value::Object(f)) => {
            if let (Some(Value::Object(_)), Some(Value::Array(log))) =
                (r.get("move_log"), f.get("move_log"))
            {
                let summary = move_log_summary(log);
                f.insert("move_log".to_owned(), summary);
            }
            for (marker, keys) in MARKED_KEYS {
                if r.contains_key(marker) && !schema.has(Part::Marker(marker)) {
                    for key in keys {
                        r.remove(*key);
                    }
                }
            }
            if f.contains_key("id") {
                // A card's `vars` stay engine-schema: an unported deck card lists none (rule 2).
                let card = f.contains_key("upgrade");
                for key in OPTIONAL_KEYS {
                    if card && key == "vars" {
                        continue;
                    }
                    if f.contains_key(key) && !r.contains_key(key) {
                        r.insert(key.to_owned(), Value::Null);
                    }
                }
            }
            for (key, rv) in r.iter_mut() {
                if let Some(fv) = f.get_mut(key) {
                    reconcile(rv, fv, schema);
                }
            }
        }
        (Value::Array(r), Value::Array(f)) => {
            for (rv, fv) in r.iter_mut().zip(f.iter_mut()) {
                reconcile(rv, fv, schema);
            }
        }
        _ => {}
    }
}

/// The card piles of a combat `state`.
const PILES: [&str; 5] = ["hand", "draw", "discard", "exhaust", "play"];

/// The first difference between a combat `state` of the engine and the referee's.
///
/// A won combat compares in full: the engine's trace already shows the torn-down state the
/// referee reports (`session::state_json`). The exceptions are leniency rules 5 to 7.
fn state_diff(rust_state: &Value, referee_state: &Value, schema: &Schema) -> Option<Difference> {
    let mut rust_state = rust_state.clone();
    if let Some(player) = rust_state.get_mut("player").and_then(Value::as_object_mut) {
        if !schema.has(Part::Potions) {
            player.remove("potions");
        }
        if !schema.has(Part::Orbs) {
            for key in ["base_orb_slots", "orb_slots", "orbs"] {
                player.remove(key);
            }
        }
    }
    if !schema.has(Part::CombatCardVars) {
        for pile in PILES {
            if let Some(Value::Array(cards)) = rust_state.get_mut(pile) {
                for card in cards.iter_mut().filter_map(Value::as_object_mut) {
                    card.remove("vars");
                    card.remove("replay");
                }
            }
        }
    }
    // Rule 6: sts2-cli claims a won fight's potion reward on its own.
    if rust_state["combat_over"] == true
        && rust_state["player_dead"] == false
        && let (Some(Value::Array(rust_belt)), Some(Value::Array(referee_belt))) = (
            rust_state["player"].get("potions").cloned().as_ref(),
            referee_state["player"].get("potions"),
        )
    {
        let belt: Vec<Value> = rust_belt
            .iter()
            .zip(referee_belt)
            .map(|(rust, referee)| {
                if rust.is_null() {
                    referee.clone()
                } else {
                    rust.clone()
                }
            })
            .collect();
        rust_state["player"]["potions"] = Value::Array(belt);
    }
    let mut referee_state = referee_state.clone();
    // Rule 7: intents once the combat is over: the engine finds a move's attacks by playing it
    // (`intent_damage`), which attacks nobody once the player is dead and cannot run after the
    // combat; the game still shows the intent. And a dead enemy's intents: the game reads them off
    // its move's static `Intents`, the engine by playing the move, which a dead creature cannot (a
    // Decimillipede segment killed while Stunned keeps its next move, PX20). The game shows no
    // intent for a dead creature.
    let over = rust_state["player_dead"] == true || rust_state["combat_over"] == true;
    for state in [&mut rust_state, &mut referee_state] {
        if let Some(Value::Array(enemies)) = state.get_mut("enemies") {
            for enemy in enemies.iter_mut().filter_map(Value::as_object_mut) {
                if over || enemy.get("alive") == Some(&Value::Bool(false)) {
                    enemy.remove("intents");
                }
            }
        }
    }
    reconcile(&mut rust_state, &mut referee_state, schema);
    diff(&rust_state, &referee_state, &mut String::new())
}

/// Walks `rust` and `referee` together; returns the first differing path and values.
/// Only the keys of `rust` are compared.
#[must_use]
pub fn diff(rust: &Value, referee: &Value, path: &mut String) -> Option<Difference> {
    match (rust, referee) {
        (Value::Object(r), Value::Object(f)) => {
            for (key, rv) in r {
                let len = path.len();
                path.push('.');
                path.push_str(key);
                let found = match f.get(key) {
                    Some(fv) => diff(rv, fv, path),
                    None => Some((path.clone(), rv.clone(), Value::Null)),
                };
                if found.is_some() {
                    return found;
                }
                path.truncate(len);
            }
            None
        }
        (Value::Array(r), Value::Array(f)) => {
            for (i, (rv, fv)) in r.iter().zip(f).enumerate() {
                let len = path.len();
                let _ = write!(path, "[{i}]");
                let found = diff(rv, fv, path);
                if found.is_some() {
                    return found;
                }
                path.truncate(len);
            }
            (r.len() != f.len()).then(|| {
                (
                    format!("{path}.len"),
                    Value::from(r.len()),
                    Value::from(f.len()),
                )
            })
        }
        _ => (rust != referee).then(|| (path.clone(), rust.clone(), referee.clone())),
    }
}

/// Leniency rule 8: the referee's run-mode `state` still dumps a fight the engine showed over
/// before it. Walking back over the engine's `{}` lines (`rust` and `referee` are the lines
/// before this one), every referee dump must be this one apart from `rng`, and the engine's last
/// line with a fight must show it over.
fn ended_fight_kept(rust: &[Value], referee: &[Value], referee_state: &Value) -> bool {
    let without_rng = |state: &Value| {
        let mut state = state.clone();
        if let Some(o) = state.as_object_mut() {
            o.remove("rng");
        }
        state
    };
    let dump = without_rng(referee_state);
    for (rust_line, referee_line) in rust.iter().zip(referee).rev() {
        if without_rng(&referee_line["state"]) != dump {
            return false;
        }
        let rust_state = &rust_line["state"];
        if !rust_state
            .as_object()
            .is_some_and(serde_json::Map::is_empty)
        {
            return rust_state["combat_over"] == true;
        }
    }
    false
}

/// Whether a referee run-mode `state` is a fight's dump (outside a fight it has only the run's
/// seed, act, floor, streams and the `combat_over`/`player_dead` flags).
fn is_fight_dump(state: &Value) -> bool {
    state.get("enemies").is_some() || state.get("player").is_some()
}

/// The first divergence of a referee run-mode trace from the engine's.
///
/// Per step it compares the `decision`, the combat `state` and the `run` object, each on the
/// engine's keys only. An engine line without a fight (`state` `{}`) diverges where the
/// referee's dumps one.
#[must_use]
pub fn first_run_divergence(rust: &[Value], referee: &[Value]) -> Option<Divergence> {
    let schema = Schema::of(referee);
    walk(rust, referee, |step, rust_line, referee_line| {
        if rust_line["decision"] != referee_line["decision"] {
            return Some((
                "decision".to_owned(),
                rust_line["decision"].clone(),
                referee_line["decision"].clone(),
            ));
        }
        let (rust_state, referee_state) = (&rust_line["state"], &referee_line["state"]);
        if rust_state
            .as_object()
            .is_some_and(serde_json::Map::is_empty)
        {
            if is_fight_dump(referee_state)
                && !ended_fight_kept(&rust[..step], &referee[..step], referee_state)
            {
                let keys = referee_state
                    .as_object()
                    .map(|o| o.keys().cloned().map(Value::from).collect())
                    .unwrap_or_default();
                return Some(("state".to_owned(), rust_state.clone(), Value::Array(keys)));
            }
        } else if let Some((path, rust, referee)) = state_diff(rust_state, referee_state, &schema) {
            return Some((format!("state{path}"), rust, referee));
        }
        let mut rust_run = rust_line["run"].clone();
        if !schema.has(Part::RunCardVars) {
            strip_card_vars(&mut rust_run);
        }
        let mut referee_run = referee_line["run"].clone();
        reconcile(&mut rust_run, &mut referee_run, &schema);
        diff(&rust_run, &referee_run, &mut String::new())
            .map(|(path, rust, referee)| (format!("run{path}"), rust, referee))
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{Difference, Schema, first_divergence, first_run_divergence, state_diff};

    fn state(card: &Value, dead: bool) -> Value {
        json!({"player_dead": dead, "hand": [card]})
    }

    /// `state_diff` with the schema of a one-line referee trace of `referee`.
    fn fight_diff(rust: &Value, referee: &Value) -> Option<Difference> {
        state_diff(rust, referee, &Schema::of(&[json!({ "state": referee })]))
    }

    #[allow(
        clippy::needless_pass_by_value,
        reason = "the tests build the values inline"
    )]
    fn fight_line(state: Value) -> Value {
        json!({ "state": state })
    }

    #[allow(
        clippy::needless_pass_by_value,
        reason = "the tests build the values inline"
    )]
    fn run_line(state: Value, run: Value) -> Value {
        json!({"decision": "map_select", "state": state, "run": run})
    }

    #[test]
    fn card_vars_and_replays_are_compared() {
        let rust = state(
            &json!({"id": "STRIKE", "vars": [["Damage", 6]], "replay": 0}),
            false,
        );
        let referee = state(
            &json!({"id": "STRIKE", "vars": [["Damage", 7]], "replay": 0}),
            false,
        );
        let (path, ..) = fight_diff(&rust, &referee).expect("a divergence");
        assert_eq!(path, ".hand[0].vars[0][1]");
        let extra = state(
            &json!({"id": "STRIKE", "vars": [["Damage", 6], ["Cards", 1]], "replay": 0}),
            false,
        );
        assert!(
            fight_diff(&rust, &extra).is_some(),
            "a var the engine lacks diverges"
        );
    }

    #[test]
    fn referee_only_keys_are_ignored() {
        let rust = json!({"player": {"hp": 10}});
        assert_eq!(
            fight_diff(&rust, &json!({"player": {"hp": 10, "gold": 99}})),
            None
        );
        assert!(fight_diff(&rust, &json!({"player": {"hp": 9, "gold": 99}})).is_some());
    }

    #[test]
    fn optional_keys_the_engine_lacks_diverge() {
        let rust = state(
            &json!({"id": "STRIKE", "upgrade": 0, "star_cost": -1}),
            false,
        );
        let referee = state(
            &json!({"id": "STRIKE", "upgrade": 0, "star_cost": -1,
                "affliction": {"id": "GALVANIZED", "amount": 1}}),
            false,
        );
        let (path, rust_value, _) = fight_diff(&rust, &referee).expect("a divergence");
        assert_eq!(path, ".hand[0].affliction");
        assert_eq!(rust_value, Value::Null);
    }

    #[test]
    fn unported_card_without_vars_is_not_compared() {
        let referee = state(
            &json!({"id": "SPLASH", "upgrade": 0, "vars": [["Damage", 6]]}),
            false,
        );
        let unported = state(&json!({"id": "SPLASH", "upgrade": 0}), false);
        assert_eq!(fight_diff(&unported, &referee), None);
        // A power's vars are an optional key: the engine must list them.
        let power = |p: Value| json!({"player": {"powers": [p]}});
        let referee = power(json!({"id": "STRENGTH", "vars": [["Amount", 1]]}));
        let (path, ..) =
            fight_diff(&power(json!({"id": "STRENGTH"})), &referee).expect("a divergence");
        assert_eq!(path, ".player.powers[0].vars");
    }

    #[test]
    fn up_front_is_compared_from_step_0() {
        let line = |up_front: i64| fight_line(json!({"rng": {"run": {"UpFront": up_front}}}));
        let rust = [line(0), line(2)];
        assert_eq!(first_divergence(&rust, &[line(400), line(402)]), None);
        let divergence = first_divergence(&rust, &[line(400), line(403)]).expect("a divergence");
        assert_eq!(
            (divergence.step, divergence.path.as_str()),
            (1, "state.rng.run.UpFront")
        );
    }

    #[test]
    fn referee_move_log_is_summarized() {
        let rust = json!({"enemies": [{"move_log": {"len": 2, "recent": ["B", "A"], "used": ["A", "B"]}}]});
        let referee = |log: Value| json!({"enemies": [{"move_log": log}]});
        assert_eq!(fight_diff(&rust, &referee(json!(["A", "B"]))), None);
        let (path, ..) = fight_diff(&rust, &referee(json!(["A", "A"]))).expect("a divergence");
        assert_eq!(path, ".enemies[0].move_log.recent[0]");
    }

    #[test]
    fn older_referee_traces_drop_what_they_lack() {
        let rust = state(
            &json!({"id": "STRIKE", "upgrade": 0, "star_cost": -1, "clone": false,
                "fields": [["x", 1]], "vars": [["Damage", 6]], "replay": 0}),
            false,
        );
        let referee = state(&json!({"id": "STRIKE", "upgrade": 0}), false);
        assert_eq!(fight_diff(&rust, &referee), None);
        let rust = json!({"player": {"hp": 5, "potions": [null], "orbs": [], "orb_slots": 0}});
        assert_eq!(fight_diff(&rust, &json!({"player": {"hp": 5}})), None);
        assert!(fight_diff(&rust, &json!({"player": {"hp": 4}})).is_some());
        let run = |deck: Value| run_line(json!({}), json!({"deck": deck}));
        let rust = [run(
            json!([{"id": "STRIKE", "upgrade": 0, "vars": [["Damage", 6]]}]),
        )];
        assert_eq!(
            first_run_divergence(&rust, &[run(json!([{"id": "STRIKE", "upgrade": 0}]))]),
            None
        );
        let divergence = first_run_divergence(
            &rust,
            &[run(
                json!([{"id": "STRIKE", "upgrade": 0, "vars": [["Damage", 7]]}]),
            )],
        )
        .expect("vars differ");
        assert_eq!(divergence.path, "run.deck[0].vars[0][1]");
    }

    /// The schema is the whole trace's: a marker key one object lacks while another has it is a
    /// divergence, not an older build (it was sniffed per object, which turned the checks off).
    #[test]
    fn a_marker_missing_from_one_object_diverges() {
        let card = json!({"id": "STRIKE", "upgrade": 0, "star_cost": -1, "clone": false});
        let rust = json!({"hand": [card, card]});
        let referee = json!({"hand": [card, {"id": "STRIKE", "upgrade": 0}]});
        let (path, ..) = fight_diff(&rust, &referee).expect("a divergence");
        assert_eq!(path, ".hand[1].star_cost");
        // Across lines too: a later line without the marker diverges.
        let line = |cards: Value| fight_line(json!({ "hand": cards }));
        let rust = [line(json!([card])), line(json!([card]))];
        let referee = [
            line(json!([card])),
            line(json!([{"id": "STRIKE", "upgrade": 0}])),
        ];
        let divergence = first_divergence(&rust, &referee).expect("a divergence");
        assert_eq!(
            (divergence.step, divergence.path.as_str()),
            (1, "state.hand[0].star_cost")
        );
        // A relic's vars in the run object are compared (not dropped with the cards').
        let run = |relic: Value| run_line(json!({}), json!({ "relic_state": [relic] }));
        let rust = [run(json!({"id": "PEAR", "vars": [["MaxHp", 10]]}))];
        let divergence =
            first_run_divergence(&rust, &[run(json!({"id": "PEAR"}))]).expect("a divergence");
        assert_eq!(divergence.path, "run.relic_state[0].vars");
    }

    #[test]
    fn won_fight_belt_takes_the_reward_potion() {
        let fight = |over: bool, belt: Value| json!({"combat_over": over, "player_dead": false, "player": {"potions": belt}});
        let referee = fight(true, json!(["FIRE_POTION", null]));
        assert_eq!(
            fight_diff(&fight(true, json!([null, null])), &referee),
            None
        );
        assert!(
            fight_diff(&fight(true, json!(["BLOCK_POTION", null])), &referee).is_some(),
            "a potion the engine holds is compared"
        );
        let ongoing = fight(false, json!(["FIRE_POTION", null]));
        assert!(
            fight_diff(&fight(false, json!([null, null])), &ongoing).is_some(),
            "only a won fight's belt is filled"
        );
    }

    #[test]
    fn intents_after_the_fight_and_of_dead_enemies_are_not_compared() {
        let fight = |dead: bool, alive: bool, intent: &str| {
            json!({"player_dead": dead, "combat_over": false,
                "enemies": [{"alive": alive, "intents": [{"type": intent}]}]})
        };
        let referee = fight(false, true, "Attack");
        assert!(fight_diff(&fight(false, true, "Buff"), &referee).is_some());
        assert_eq!(
            fight_diff(&fight(true, true, "Buff"), &fight(true, true, "Attack")),
            None
        );
        assert_eq!(
            fight_diff(&fight(false, false, "Buff"), &fight(false, false, "Attack")),
            None
        );
    }

    #[test]
    fn traces_must_line_up() {
        let line = |hp: i64| fight_line(json!({"player": {"hp": hp}}));
        let divergence = first_divergence(&[line(1)], &[line(1), line(1)]).expect("extra line");
        assert_eq!((divergence.step, divergence.path.as_str()), (1, "step"));
        let divergence = first_divergence(&[line(1), line(1)], &[line(1)]).expect("missing line");
        assert_eq!((divergence.step, divergence.path.as_str()), (1, "step"));
        let divergence = first_divergence(&[], &[]).expect("empty trace");
        assert_eq!(divergence.path, "trace.len");
        let divergence =
            first_divergence(&[line(1)], &[json!({"error": "rejected"})]).expect("error line");
        assert_eq!(divergence.path, "error");
        let run = |state: Value| run_line(state, json!({}));
        assert!(
            first_run_divergence(&[run(json!({}))], &[run(json!({})), run(json!({}))]).is_some()
        );
    }

    #[test]
    fn run_mode_compares_an_empty_engine_state() {
        let run = |state: Value| run_line(state, json!({}));
        let outside =
            json!({"act": "OVERGROWTH", "floor": 1, "combat_over": false, "player_dead": false});
        assert_eq!(
            first_run_divergence(&[run(json!({}))], &[run(outside)]),
            None
        );
        let fight = json!({"combat_over": false, "player": {"hp": 5}, "enemies": []});
        let divergence = first_run_divergence(&[run(json!({}))], &[run(fight.clone())])
            .expect("the referee is in a fight");
        assert_eq!(divergence.path, "state");
        assert_eq!(
            first_run_divergence(&[run(json!({"player": {"hp": 5}}))], &[run(fight)]),
            None
        );
        // Rule 8: the referee keeps the ended fight's dump on the next line (a rerolled reward).
        let ended = |rng: i64, hp: i64| json!({"combat_over": true, "rng": {"run": {"Rewards": rng}}, "player": {"hp": hp}, "enemies": []});
        let rust = [run(ended(1, 5)), run(json!({})), run(json!({}))];
        assert_eq!(
            first_run_divergence(
                &rust,
                &[run(ended(1, 5)), run(ended(2, 5)), run(ended(3, 5))]
            ),
            None
        );
        let rust = [run(ended(1, 5)), run(json!({}))];
        let divergence = first_run_divergence(&rust, &[run(ended(1, 5)), run(ended(2, 4))])
            .expect("the dump changed");
        assert_eq!((divergence.step, divergence.path.as_str()), (1, "state"));
        let ongoing = json!({"combat_over": false, "player": {"hp": 5}, "enemies": []});
        let rust = [run(ongoing.clone()), run(json!({}))];
        assert!(
            first_run_divergence(&rust, &[run(ongoing.clone()), run(ongoing)]).is_some(),
            "the engine's previous line must show the fight over"
        );
    }
}
