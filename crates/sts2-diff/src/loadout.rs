//! Custom loadouts: the referee's `loadout` object applied to a [`CombatConfig`].
//!
//! `{"deck": ["STRIKE_NECROBINDER", "BODYGUARD+1"], "relics": [...], "potions": [...],
//! "hp": 40, "max_hp": 66, "character": "Necrobinder"}`. Every field is optional; a missing one
//! keeps the starter value. `character` (default Necrobinder) is read by `sts2-diff gen`, which
//! passes it to the referee; the engine itself plays Necrobinder until the core takes a character.
//! Ids are the referee's; a `+N` suffix upgrades a card `N` times, and a `:ENCHANTMENT` or
//! `:ENCHANTMENT*AMOUNT` suffix after it enchants the card (`CardCmd.Enchant`, amount 1 by
//! default): `"STRIKE_NECROBINDER+1:VIGOROUS*3"`. A final `#N` sets the card's saved counter
//! (`Guilty.CombatsSeen`, `Dowsing.RoomsEntered`): `"GUILTY#2"`.
//!
//! A run loadout ([`apply_run`]) is added to a run that waits on the map after Neow:
//! `{"act_index": N, "remove_relics": [...], "remove_cards": [...], "add_cards": [...], "gold": N,
//! "add_relics": [...], "max_hp": N, "hp": N}`, in that order. `act_index` first enters that act
//! (`RunManager.EnterAct`, [`Run::debug_enter_act`]), so an act-gated event can be forced in its
//! own act (and the act's ancient is its first room). `remove_relics` takes each id's first relic
//! off and `remove_cards` every deck card of each id, without hooks (the referee's
//! `RemoveRelicInternal`, `RemoveInternal`), so an ancient can be entered without a starter.
//! `run-gen` takes `"event"` and `"ancient"` out of the loadout first (forced event, stop after the
//! ancient); other keys (`policy`, `covers`, ...) are for `tools/regress_runs.sh`.
//!
//! A key outside [`LOADOUT_KEYS`] or [`RUN_LOADOUT_KEYS`] is an error: a typo (`"relic"` for
//! `"relics"`) used to test the starter deck silently.
//! The cards are not run through the deck hooks (the referee adds them with `AddInternal`);
//! each relic is obtained like the game does (`RelicCmd.Obtain`), so its `AfterObtained` runs and
//! a prompt it opens (`DollysMirror`) is the run's next decision, which only the last relic may do.

use serde_json::Value;
use sts2_core::CombatConfig;
use sts2_core::combat::DeckCard;
use sts2_core::content::enchantments::{Enchantment, EnchantmentKind};
use sts2_core::run::{Run, RunCard, RunDecision};

use crate::{Result, ids};

/// The keys of a combat loadout: what [`apply`] reads, `character` (`sts2-diff gen`), and what
/// `tools/regress.sh` reads (`encounters`, `ascensions`) or a reader needs (`note`).
pub const LOADOUT_KEYS: [&str; 9] = [
    "deck",
    "relics",
    "potions",
    "hp",
    "max_hp",
    "character",
    "encounters",
    "ascensions",
    "note",
];

/// The keys of a run loadout.
///
/// They are what [`apply_run`] reads, what `sts2-diff run-gen` reads (`event`, `continue`,
/// `ancient`, `act1`), and what `tools/regress_runs.sh` and `tools/regress_plan.py` read
/// (`policy`, `seed`, `covers`, `prefer`, `characters`, `acts`, `ascensions`) or a reader needs
/// (`note`).
pub const RUN_LOADOUT_KEYS: [&str; 20] = [
    "act_index",
    "remove_relics",
    "remove_cards",
    "add_cards",
    "gold",
    "add_relics",
    "max_hp",
    "hp",
    "event",
    "continue",
    "ancient",
    "act1",
    "policy",
    "seed",
    "covers",
    "prefer",
    "characters",
    "acts",
    "ascensions",
    "note",
];

/// An error for the first key of `loadout` (an object) that is not in `known`.
fn known_keys(loadout: &Value, known: &[&str], what: &str) -> Result<()> {
    let object = loadout
        .as_object()
        .ok_or_else(|| format!("{what} is not an object"))?;
    object
        .keys()
        .find(|k| !known.contains(&k.as_str()))
        .map_or(Ok(()), |key| {
            Err(format!(
                "{what} has an unknown key {key:?} (known: {})",
                known.join(", ")
            )
            .into())
        })
}

/// Applies `loadout` to `config`.
///
/// # Errors
/// If a key is unknown, a field is malformed or names an id the engine does not have.
pub fn apply(config: &mut CombatConfig, loadout: &Value) -> Result<()> {
    known_keys(loadout, &LOADOUT_KEYS, "loadout")?;
    if let Some(deck) = loadout.get("deck") {
        config.deck = strings(deck, "deck")?
            .into_iter()
            .map(deck_card)
            .collect::<Result<_>>()?;
    }
    if let Some(relics) = loadout.get("relics") {
        config.relics = strings(relics, "relics")?
            .into_iter()
            .map(|id| {
                ids::relic_from_id(id)
                    .map(Into::into)
                    .ok_or_else(|| format!("unknown relic {id}").into())
            })
            .collect::<Result<_>>()?;
    }
    if let Some(potions) = loadout.get("potions") {
        config.potions = strings(potions, "potions")?
            .into_iter()
            .map(|id| {
                ids::potion_from_id(id)
                    .map(|k| Some(k.into()))
                    .ok_or_else(|| format!("unknown potion {id}").into())
            })
            .collect::<Result<_>>()?;
        if config.potions.len() > config.potion_slots {
            return Err(format!(
                "{} potions do not fit in {} slots",
                config.potions.len(),
                config.potion_slots
            )
            .into());
        }
    }
    if let Some(max_hp) = loadout.get("max_hp") {
        config.player_max_hp = int(max_hp, "max_hp")?;
        config.player_hp = config.player_hp.min(config.player_max_hp);
    }
    if let Some(hp) = loadout.get("hp") {
        config.player_hp = int(hp, "hp")?;
    }
    Ok(())
}

fn strings<'a>(value: &'a Value, field: &str) -> Result<Vec<&'a str>> {
    value
        .as_array()
        .ok_or_else(|| format!("loadout {field} is not a list"))?
        .iter()
        .map(|v| {
            v.as_str()
                .ok_or_else(|| format!("loadout {field} has a non-string entry {v}").into())
        })
        .collect()
}

fn int(value: &Value, field: &str) -> Result<i32> {
    value
        .as_i64()
        .and_then(|n| i32::try_from(n).ok())
        .ok_or_else(|| format!("loadout {field} is not an integer: {value}").into())
}

/// `STRIKE_NECROBINDER`, `BODYGUARD+1`, `STRIKE_NECROBINDER+1:VIGOROUS*3` or `GUILTY#2`.
fn deck_card(id: &str) -> Result<DeckCard> {
    let (id, counter) = match id.split_once('#') {
        Some((id, n)) => (
            id,
            n.parse().map_err(|_| format!("bad counter in {id}#{n}"))?,
        ),
        None => (id, 0),
    };
    let (id, enchantment) = match id.split_once(':') {
        Some((id, spec)) => (id, Some(enchantment(spec)?)),
        None => (id, None),
    };
    let (id, upgrades) = match id.split_once('+') {
        Some((id, n)) => (
            id,
            n.parse()
                .map_err(|_| format!("bad upgrade count in {id}+{n}"))?,
        ),
        None => (id, 0),
    };
    let kind = ids::card_from_id(id).ok_or_else(|| format!("unknown card {id}"))?;
    Ok(DeckCard {
        kind,
        upgrade_level: upgrades,
        enchantment,
        counter,
    })
}

/// `VIGOROUS` or `VIGOROUS*3`: the enchantment a deck card starts with.
fn enchantment(spec: &str) -> Result<Enchantment> {
    let (id, amount) = match spec.split_once('*') {
        Some((id, n)) => (
            id,
            n.parse()
                .map_err(|_| format!("bad enchantment amount in {spec}"))?,
        ),
        None => (spec, 1),
    };
    let kind = EnchantmentKind::ALL
        .into_iter()
        .find(|k| k.id() == id)
        .ok_or_else(|| format!("unknown enchantment {id}"))?;
    Ok(Enchantment::new(kind, amount))
}

/// Applies the run loadout `loadout` to `run` (see the module docs). Returns whether a relic's
/// prompt is now pending.
///
/// # Errors
/// If a key is unknown, a field is malformed, names an id the engine does not have, or the run is not on the map
/// or a relic other than the last one prompts.
pub fn apply_run(run: &mut Run, loadout: &Value) -> Result<bool> {
    known_keys(loadout, &RUN_LOADOUT_KEYS, "run loadout")?;
    let cards = loadout.get("add_cards").map_or(Ok(Vec::new()), |v| {
        strings(v, "add_cards")?
            .into_iter()
            .map(deck_card)
            .collect::<Result<Vec<_>>>()
    })?;
    let relics = loadout.get("add_relics").map_or(Ok(Vec::new()), |v| {
        strings(v, "add_relics")?
            .into_iter()
            .map(|id| {
                ids::relic_from_id(id)
                    .map(ids::relic)
                    .ok_or_else(|| format!("unknown relic {id}").into())
            })
            .collect::<Result<Vec<_>>>()
    })?;
    if let Some(index) = loadout.get("act_index") {
        let index = int(index, "act_index")?;
        run.debug_enter_act(usize::try_from(index).map_err(|_| "act_index is negative")?)
            .map_err(|e| format!("act_index {index}: {e}"))?;
    }
    let state = run.loadout_state().map_err(|e| e.to_string())?;
    if let Some(ids) = loadout.get("remove_relics") {
        for id in strings(ids, "remove_relics")? {
            let index = state
                .relics
                .iter()
                .position(|r| r.kind.id() == id)
                .ok_or_else(|| format!("remove_relics: no relic {id}"))?;
            state.relics.remove(index);
        }
    }
    if let Some(ids) = loadout.get("remove_cards") {
        for id in strings(ids, "remove_cards")? {
            let before = state.deck.len();
            state.deck.retain(|c| c.id != id);
            if state.deck.len() == before {
                return Err(format!("remove_cards: no card {id}").into());
            }
        }
    }
    for card in cards {
        let mut run_card = RunCard::new(card.kind.entry());
        run_card.upgrade_level = card.upgrade_level;
        run_card.enchantment = card.enchantment;
        run_card.counter = card.counter;
        state.deck.push(run_card);
    }
    if let Some(gold) = loadout.get("gold") {
        state.gold = int(gold, "gold")?;
    }
    for id in relics {
        if !matches!(run.pending_decision(), RunDecision::Map(_)) {
            return Err(format!("relic {id} follows a relic that prompts").into());
        }
        run.loadout_relic(id).map_err(|e| e.to_string())?;
    }
    let prompting = !matches!(run.pending_decision(), RunDecision::Map(_));
    let sets_hp = loadout.get("max_hp").is_some() || loadout.get("hp").is_some();
    if prompting && sets_hp {
        return Err("hp and max_hp cannot follow a relic that prompts".into());
    }
    if sets_hp {
        let state = run.loadout_state().map_err(|e| e.to_string())?;
        if let Some(max_hp) = loadout.get("max_hp") {
            state.max_hp = int(max_hp, "max_hp")?;
            state.hp = state.hp.min(state.max_hp);
        }
        if let Some(hp) = loadout.get("hp") {
            state.hp = int(hp, "hp")?;
        }
    }
    Ok(prompting)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sts2_core::CombatConfig;

    #[test]
    fn unknown_loadout_keys_are_errors() {
        let mut config = CombatConfig::necrobinder("S", 0);
        assert!(super::apply(&mut config, &json!({"relic": ["ANCHOR"]})).is_err());
        assert!(super::apply(&mut config, &json!({"hp": 20, "note": "why"})).is_ok());
    }
}
