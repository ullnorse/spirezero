//! Plays the engine in the referee's terms: referee-format actions in, referee-schema states out.

use serde_json::{Value, json};
use sts2_core::combat::{
    CardId, CombatResult, CombatState, CreatureId, Decision, DecisionRequest, Encounter, Side,
};
use sts2_core::content::cards::{CardKeyword, CardKind};
use sts2_core::decimal::Decimal;
use sts2_core::rng::RunRngType;
use sts2_core::{Action, Character, Combat, CombatConfig, Status};

use sts2_core::content::enchantments::EnchantmentStatus;

use crate::{Result, fields, ids, loadout};

/// The character used when none is given.
pub const DEFAULT_CHARACTER: Character = Character::Necrobinder;
/// The encounter used when none is given.
pub const DEFAULT_ENCOUNTER: Encounter = Encounter::FuzzyWurmCrawlerWeak;
/// The ascension used when none is given.
pub const ASCENSION: u8 = 0;

/// The character with class name `name` (`Ironclad`), or [`DEFAULT_CHARACTER`] for `None`.
///
/// # Errors
/// If `name` names no character.
pub fn character_or_default(name: Option<&str>) -> Result<Character> {
    name.map_or(Ok(DEFAULT_CHARACTER), |name| {
        Character::from_name(name).ok_or_else(|| format!("unknown character {name}").into())
    })
}

/// The encounter with referee id `id`, or [`DEFAULT_ENCOUNTER`] for `None`.
///
/// # Errors
/// If `id` names an encounter the engine does not have.
pub fn encounter_or_default(id: Option<&str>) -> Result<Encounter> {
    id.map_or(Ok(DEFAULT_ENCOUNTER), |id| {
        ids::encounter_from_id(id).ok_or_else(|| format!("unknown encounter {id}").into())
    })
}

/// A combat plus the seed and encounter it was started from.
#[derive(Debug)]
pub struct Session {
    character: Character,
    seed: String,
    encounter: Encounter,
    ascension: u8,
    combat: Combat,
    step: u64,
}

impl Session {
    /// A fresh combat of `character` at `ascension` against `encounter` on `seed`, with the
    /// starter loadout changed by the referee-format `loadout` object, if any.
    ///
    /// # Errors
    /// If a starter card is not ported, or the loadout is malformed or names something the
    /// engine does not have.
    pub fn new(
        character: Character,
        seed: &str,
        encounter: Encounter,
        ascension: u8,
        loadout: Option<&Value>,
    ) -> Result<Self> {
        let mut config = CombatConfig::new(character, seed, ascension)
            .map_err(|u| format!("{} starts with unported {u}", character.name()))?
            .with_encounter(encounter)
            .with_events();
        if let Some(loadout) = loadout {
            loadout::apply(&mut config, loadout)?;
        }
        Ok(Self {
            character,
            seed: seed.to_owned(),
            encounter,
            ascension,
            combat: Combat::new(config),
            step: 0,
        })
    }

    /// The combat.
    #[must_use]
    pub const fn combat(&self) -> &Combat {
        &self.combat
    }

    /// Whether the combat has ended.
    #[must_use]
    pub const fn is_over(&self) -> bool {
        matches!(self.combat.status(), Status::Won | Status::Lost)
    }

    /// Whether the harness never takes `action`: it plays a card the referee cannot run
    /// ([`referee_broken_play`]), or it would stop the combat at unported content
    /// ([`Status::Unsupported`], tried on a copy).
    #[must_use]
    pub fn stops_at(&self, action: &Action) -> bool {
        if referee_broken_play(&self.combat, action).is_some() {
            return true;
        }
        let mut copy = self.combat.clone();
        matches!(copy.apply(action.clone()), Ok(Status::Unsupported(_)))
    }

    /// The trace line for step 0, the state at the start of the fight.
    #[must_use]
    pub fn start_line(&self) -> Value {
        json!({
            "step": 0,
            "action": {
                "action": "start",
                "seed": self.seed,
                "character": self.character.name(),
                "ascension": self.ascension,
                "encounter": ids::encounter(self.encounter),
            },
            "state": forced_fight_state_json(self.combat.state()),
        })
    }

    /// Applies an engine action and returns the referee-format action and the trace line.
    ///
    /// # Errors
    /// If the engine rejects the action or it has no referee equivalent.
    pub fn apply(&mut self, action: &Action) -> Result<(Value, Value)> {
        let referee_action = to_referee(&self.combat, action)?;
        self.combat.apply(action.clone())?;
        self.step += 1;
        crate::capacity::observe(&self.combat, || format!("seed {}", self.seed));
        let line = json!({
            "step": self.step,
            "action": referee_action,
            "state": forced_fight_state_json(self.combat.state()),
        });
        Ok((referee_action, line))
    }

    /// Applies a referee-format action and returns the trace line.
    ///
    /// # Errors
    /// If the action is malformed, unsupported, or illegal.
    pub fn apply_referee(&mut self, action: &Value) -> Result<Value> {
        let action = from_referee(&self.combat, action)?;
        Ok(self.apply(&action)?.1)
    }
}

/// The trace of a whole referee actions list on the engine, step 0 first. Stops early (without
/// error) once the combat is over, like the referee does.
///
/// # Errors
/// If an action cannot be applied.
pub fn replay(
    character: Character,
    seed: &str,
    encounter: Encounter,
    ascension: u8,
    loadout: Option<&Value>,
    actions: &[Value],
) -> Result<Vec<Value>> {
    let mut session = Session::new(character, seed, encounter, ascension, loadout)?;
    let mut trace = vec![session.start_line()];
    for action in actions {
        if session.is_over() {
            break;
        }
        trace.push(session.apply_referee(action)?);
    }
    Ok(trace)
}

/// An engine action as a referee action.
///
/// # Errors
/// For an action that does not fit the combat (a card not in hand, a dead target, a decision
/// with no pending request).
pub fn to_referee(combat: &Combat, action: &Action) -> Result<Value> {
    let state = combat.state();
    match action {
        Action::EndTurn => Ok(json!({"action": "end_turn"})),
        Action::PlayCard { card, target } => {
            let card_index = state
                .player
                .hand
                .iter()
                .position(|c| c == card)
                .ok_or("card is not in hand")?;
            let mut args = json!({"card_index": card_index});
            if let Some(target) = target {
                let target_index = living_enemies(state)
                    .position(|e| e == *target)
                    .ok_or("target is not a living enemy")?;
                args["target_index"] = json!(target_index);
            }
            Ok(json!({"action": "play_card", "args": args}))
        }
        Action::UsePotion { slot, target } => {
            // `potion_index` counts the potions in the belt, not slots (`RunSimulator.cs:2045`).
            let potion_index = state.player.potions[..*slot].iter().flatten().count();
            let mut args = json!({"potion_index": potion_index});
            // The referee picks the player itself for player-targeted potions; it only reads
            // `target_index` (over living enemies) for enemy-targeted ones.
            if let Some(target) = target
                && state.creature(*target).side == Side::Enemy
            {
                let target_index = living_enemies(state)
                    .position(|e| e == *target)
                    .ok_or("target is not a living enemy")?;
                args["target_index"] = json!(target_index);
            }
            Ok(json!({"action": "use_potion", "args": args}))
        }
        Action::Resolve(Decision::Cards(cards)) => {
            let options = referee_options(combat)?;
            let indices = cards
                .iter()
                .map(|c| {
                    options
                        .iter()
                        .position(|o| o == c)
                        .map(|i| i.to_string())
                        .ok_or("chosen card is not an option")
                })
                .collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(json!({"action": "select_cards", "args": {"indices": indices.join(",")}}))
        }
        Action::Resolve(Decision::Pick(_) | Decision::Unpick(_) | Decision::Done) => {
            Err("the referee takes a whole selection: use `whole_choice`".into())
        }
    }
}

/// Cards the referee cannot play: none at the moment.
///
/// A game method that references a Godot member sts2-cli's stubs lack throws
/// `MissingMethodException` as a whole (`RollingBoulder` was lost this way until
/// `referee/stubs` added `GodotObject.Connect`); `referee/stubcheck` fails the referee build on
/// such a method, so list a card here only while its stub is being added.
pub const REFEREE_BROKEN_CARDS: [CardKind; 0] = [];

/// The card `action` plays if the referee cannot run it.
///
/// That is a card of [`REFEREE_BROKEN_CARDS`], or a `VoidForm` whose own end of turn (`PlayerCmd.EndTurn` inside `OnPlay`, `VoidForm.cs`) wins
/// the fight: the game's `PlayCardAction` then finds itself in no queue
/// (`ActionQueueSet.PopAction` throws) and the faulted turn loop ends the next fight at its start
/// (`CombatManager.RunTurnLoopAfter`), so the referee is no oracle from there (PX15, `RL2S157`).
/// Or a `VoidForm` whose end of turn opens a choice at the next turn's start (`EntropyPower`'s
/// transform): sts2-cli does not surface that prompt while the play is still running and its
/// `CombatPlayState` dump throws `Collection was modified` on the hand (PX20, Necrobinder A10
/// `ROT261023S3`).
#[must_use]
pub fn referee_broken_play(combat: &Combat, action: &Action) -> Option<CardKind> {
    let Action::PlayCard { card, .. } = action else {
        return None;
    };
    let kind = combat.state().card(*card).kind;
    let breaks_referee = || {
        let mut copy = combat.clone();
        copy.apply(action.clone()).is_ok()
            && (copy.status() == Status::Won || copy.pending_decision().is_some())
    };
    (REFEREE_BROKEN_CARDS.contains(&kind) || (kind == CardKind::VoidForm && breaks_referee()))
        .then_some(kind)
}

pub use sts2_rl::resolve::whole_choice;

/// The options of the pending card choice in the referee's order: its selector lists a draw
/// pile ordered by rarity, then id (`CardSelectCmd.cs:489-494`, a stable `OrderBy`), and any
/// other pile in pile order.
///
/// # Errors
/// If no card choice is pending.
pub fn referee_options(combat: &Combat) -> Result<Vec<CardId>> {
    let Some(DecisionRequest::ChooseCards { options, .. }) = combat.pending_decision() else {
        return Err("no card choice is pending".into());
    };
    Ok(referee_order(combat.state(), options))
}

/// `options`, the cards of a card choice in `state`, in the referee's order (see [`referee_options`]).
#[must_use]
pub fn referee_order(state: &CombatState, options: &[CardId]) -> Vec<CardId> {
    let mut options = options.to_vec();
    if options.iter().all(|c| state.player.draw.contains(c)) {
        options.sort_by_key(|&c| {
            let kind = state.card(c).kind;
            (ids::card_rarity(kind), ids::card(kind))
        });
    }
    options
}

/// A referee action as an engine action.
///
/// # Errors
/// If the action is malformed or not one the engine models.
pub fn from_referee(combat: &Combat, action: &Value) -> Result<Action> {
    let state = combat.state();
    match action["action"].as_str() {
        Some("select_cards") => {
            let options = referee_options(combat)?;
            let cards = action["args"]["indices"]
                .as_str()
                .ok_or("select_cards needs indices")?
                .split(',')
                .filter(|i| !i.is_empty())
                .map(|i| -> Result<CardId> {
                    let i: usize = i.trim().parse()?;
                    Ok(*options.get(i).ok_or("select_cards index is out of range")?)
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(Action::Resolve(Decision::Cards(cards)))
        }
        Some("end_turn") => Ok(Action::EndTurn),
        Some("play_card") => {
            let card_index =
                arg_index(action, "card_index")?.ok_or("play_card needs card_index")?;
            let card = *state
                .player
                .hand
                .get(card_index)
                .ok_or("card_index is out of range")?;
            let target = arg_index(action, "target_index")?
                .map(|i| {
                    living_enemies(state)
                        .nth(i)
                        .ok_or("target_index is out of range")
                })
                .transpose()?;
            Ok(Action::PlayCard { card, target })
        }
        Some("use_potion") => {
            let potion_index =
                arg_index(action, "potion_index")?.ok_or("use_potion needs potion_index")?;
            let slot = state
                .player
                .potions
                .iter()
                .enumerate()
                .filter(|(_, p)| p.is_some())
                .nth(potion_index)
                .map(|(slot, _)| slot)
                .ok_or("potion_index is out of range")?;
            let target = arg_index(action, "target_index")?
                .map(|i| {
                    living_enemies(state)
                        .nth(i)
                        .ok_or("target_index is out of range")
                })
                .transpose()?;
            Ok(Action::UsePotion { slot, target })
        }
        other => Err(format!("unsupported referee action {other:?}").into()),
    }
}

/// The optional index argument `key` of a referee action.
fn arg_index(action: &Value, key: &str) -> Result<Option<usize>> {
    action["args"]
        .get(key)
        .map(|v| {
            v.as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or_else(|| format!("{key} is not an index").into())
        })
        .transpose()
}

/// The referee's `target_index` counts living enemies only (`RunSimulator.cs:1017`).
fn living_enemies(state: &CombatState) -> impl Iterator<Item = CreatureId> + '_ {
    state
        .enemies
        .iter()
        .copied()
        .filter(|&e| state.creature(e).is_alive())
}

/// The card keywords the trace lists, sorted by name like the referee's ordinal sort.
const KEYWORDS: [(CardKeyword, &str); 7] = [
    (CardKeyword::Eternal, "Eternal"),
    (CardKeyword::Ethereal, "Ethereal"),
    (CardKeyword::Exhaust, "Exhaust"),
    (CardKeyword::Innate, "Innate"),
    (CardKeyword::Retain, "Retain"),
    (CardKeyword::Sly, "Sly"),
    (CardKeyword::Unplayable, "Unplayable"),
];

/// A card's `DynamicVars` in the referee's schema (`TraceDump.cs` `Vars`): `[name, BaseValue]` in
/// declaration order, plus the untargeted `Calculate(null)` for a calculated var.
fn vars_json(s: &CombatState, card: CardId) -> Vec<Value> {
    let c = s.card(card);
    c.kind
        .vars()
        .iter()
        .map(|v| {
            let mut entry = vec![json!(v.name.to_string()), decimal_json(c.var(v.name))];
            if v.multiplier.is_some()
                && let Some(value) = s.calculate(card, v.name, None)
            {
                entry.push(decimal_json(value));
            }
            Value::Array(entry)
        })
        .collect()
}

/// The state of a forced fight (no run around it).
///
/// A won fight whose encounter gives no rewards (`ShouldGiveRewards` false, the `BattlewornDummy` fights) goes straight back to the map
/// (`NCombatUi.OnCombatWon` -> `ProceedWithoutRewards`), where the referee has no combat to dump and
/// reports only the flags, like a boss kill that leaves the room (PX17).
#[must_use]
pub fn forced_fight_state_json(s: &CombatState) -> Value {
    if s.result == Some(CombatResult::Won) && !s.encounter.should_give_rewards() {
        return json!({"combat_over": true, "player_dead": false});
    }
    state_json(s)
}

/// The engine state in the referee's `state` schema, restricted to the fields the engine models.
///
/// A won combat is shown the way the referee reports it once the room is torn down
/// (`CombatManager.EndCombatInternal`, `Player.AfterCombatEnd`, `PlayerCombatState.AfterCombatEnd`):
/// the player's powers and block removed, every pile emptied, Osty gone. The
/// rest (HP, energy, potions, RNG counters) is the engine's own, so end-of-combat effects show.
/// A lost combat is not torn down.
#[must_use]
pub fn state_json(s: &CombatState) -> Value {
    let player = s.creature(s.player.creature);
    let won = s.result == Some(CombatResult::Won);
    let pile = |cards: &[CardId]| -> Vec<Value> {
        if won {
            return Vec::new();
        }
        cards.iter().map(|&c| card_json(s, c)).collect()
    };
    // The referee lists `CombatState.Enemies` (`TraceDump.cs:65`): dead enemies drop out, and a
    // won combat's teardown does not clear the list, so a dead creature that stays in combat
    // after death (a Fogmog's Eye With Teeth waiting to revive) is still listed.
    let enemies: Vec<Value> = s
        .enemies
        .iter()
        .map(|&e| s.creature(e))
        .filter_map(|c| {
            let monster = c.monster.as_ref()?;
            let id = ids::enemy(&monster.kind)?;
            Some(json!({
                "id": id,
                "alive": c.is_alive(),
                "hp": c.hp,
                "max_hp": c.max_hp,
                "block": c.block,
                "next_move": monster.next_move_name(),
                "spawned_this_turn": monster.spawned_this_turn,
                "performed_first_move": monster.machine.performed_first_move,
                "move_log": move_log_json(monster),
                "intents": intents_json(s, c.id),
                "powers": powers_json(s, c.id),
            }))
        })
        .collect();
    // `player.Osty == null` (no pet yet, or a character without one) is `{"alive": false}`.
    let osty = s.osty().filter(|_| !won).map_or_else(
        || json!({"alive": false}),
        |id| {
            let c = s.creature(id);
            json!({
                "alive": c.is_alive(),
                "hp": c.hp,
                "max_hp": c.max_hp,
                "block": c.block,
                "powers": powers_json(s, id),
            })
        },
    );
    let counters: serde_json::Map<String, Value> = RunRngType::ALL
        .iter()
        .map(|&t| (t.enum_name().to_owned(), json!(s.rng[t].counter())))
        .collect();
    json!({
        "combat_over": s.result.is_some(),
        "player_dead": s.result == Some(CombatResult::Lost) || player.is_dead(),
        "round": s.round,
        "side": match s.side { Side::Player => "Player", Side::Enemy => "Enemy" },
        "rng": {"run": counters},
        "player": {
            "hp": player.hp,
            "max_hp": player.max_hp,
            "block": if won { 0 } else { player.block },
            "energy": s.player.energy,
            "stars": s.player.stars,
            "base_orb_slots": s.player.character.base_orb_slots(),
            "orb_slots": s.player.orb_slots,
            "orbs": orbs_json(s),
            "max_energy": s.player.max_energy,
            "turn_number": s.player.turn_number,
            "pets": s
                .player
                .pets
                .iter()
                .map(|&p| s.creature(p).monster.as_ref().map(|m| m.kind.id()))
                .collect::<Vec<_>>(),
            "relics": s
                .player
                .relics
                .iter()
                .map(|r| fields::relic_json(Some(s), r.kind, &r.data))
                .collect::<Vec<_>>(),
            "powers": if won { Vec::new() } else { powers_json(s, player.id) },
            "potions": s
                .player
                .potions
                .iter()
                .map(|p| p.map(|p| ids::potion(p.kind)))
                .collect::<Vec<_>>(),
        },
        "osty": osty,
        "hand": pile(&s.player.hand),
        "draw": pile(&s.player.draw),
        "discard": pile(&s.player.discard),
        "exhaust": pile(&s.player.exhaust),
        "play": pile(&s.player.play),
        "enemies": enemies,
    })
}

/// A combat card in the referee's schema (`TraceDump.cs` `Cards`).
fn card_json(s: &CombatState, c: CardId) -> Value {
    let card = s.card(c);
    let card_keywords = s.keywords(c);
    let keywords: Vec<&str> = KEYWORDS
        .iter()
        .filter(|(k, _)| card_keywords.contains(*k))
        .map(|&(_, name)| name)
        .collect();
    let mut json = json!({
        "id": ids::card(card.kind),
        "upgrade": card.upgrade_level,
        "cost": s.energy_to_spend(c),
        "keywords": keywords,
        "exhaust_on_next_play": card.exhaust_on_next_play,
        "retain": s.should_retain_this_turn(c),
        "sly": s.is_sly_this_turn(c),
        "vars": vars_json(s, c),
        "replay": card.base_replay_count,
        "star_cost": s.star_cost(c),
        "last_stars": card.star_cost.last_spent,
        "clone": card.is_clone(),
    });
    // Listed only when present (`TraceDump.cs`).
    if let Some(e) = card.enchantment {
        let status = match e.status {
            EnchantmentStatus::Normal => "Normal",
            EnchantmentStatus::Disabled => "Disabled",
        };
        json["enchantment"] = json!({"id": e.kind.id(), "amount": e.amount, "status": status});
        if let Some(f) = fields::enchantment(&e) {
            json["enchantment"]["fields"] = f;
        }
    }
    if let Some(a) = card.affliction {
        json["affliction"] = json!({"id": a.kind.id(), "amount": a.amount});
    }
    if let Some(f) = fields::card(card.kind, &card.data) {
        json["fields"] = f;
    }
    json
}

/// `OrbQueue.Orbs` with each orb's `PassiveVal` and `EvokeVal` now (`TraceDump.cs`).
fn orbs_json(s: &CombatState) -> Vec<Value> {
    s.player
        .orbs
        .iter()
        .map(|&o| {
            json!({
                "id": o.kind.id(),
                "passive": decimal_json(s.orb_passive_val(o)),
                "evoke": decimal_json(s.orb_evoke_val(o)),
            })
        })
        .collect()
}

/// A C# `decimal` as the referee writes it: a JSON integer when it has no fraction.
#[must_use]
pub fn decimal_json(d: Decimal) -> Value {
    if d == d.truncate() {
        json!(d.to_int_truncating())
    } else {
        json!(d.to_string())
    }
}

/// A creature's powers in the referee's schema (`TraceDump.cs` `Powers`).
fn powers_json(s: &CombatState, id: CreatureId) -> Vec<Value> {
    s.creature(id)
        .powers
        .iter()
        .map(|p| {
            let mut json = json!({
                "id": ids::power(p.kind),
                "amount": p.amount,
                "amount_on_turn_start": p.amount_on_turn_start,
                "skip_tick": p.skip_next_duration_tick,
                "applier": fields::creature_ref(s, p.applier),
                "target": fields::power_target(s, s.power_data(p.id)),
            });
            if let Some(vars) = fields::power_vars(s.power_data(p.id)) {
                json["vars"] = vars;
            }
            if let Some(f) = fields::power(s, s.power_data(p.id)) {
                json["fields"] = f;
            }
            json
        })
        .collect()
}

/// An enemy's `StateLog` as the engine keeps it (`MoveLog`): the number of entries, the last
/// three ids (newest first) and the sorted set of ids it ever held. `compare` reduces the
/// referee's full log to the same form.
fn move_log_json(monster: &sts2_core::content::monsters::Monster) -> Value {
    let states = monster.kind.moves().states;
    let log = &monster.machine.log;
    let mut used: Vec<&str> = (0..states.len())
        .filter(|&i| log.contains(i))
        .map(|i| states[i].id())
        .collect();
    used.sort_unstable();
    used.dedup();
    json!({
        "len": log.len(),
        "recent": log.recent(3).map(|i| states[i].id()).collect::<Vec<_>>(),
        "used": used,
    })
}

/// The attack intents of a monster's next move (`TraceDump.cs` `Intents`).
/// Empty once the combat is over (the move can no longer be played out; `compare` leaves them
/// out then).
fn intents_json(s: &CombatState, id: CreatureId) -> Vec<Value> {
    if s.result.is_some() {
        return Vec::new();
    }
    s.intent_damage(id)
        .iter()
        // `IntentType`'s variants carry the C# names (`Attack`, `DeathBlow`).
        .map(|a| json!({"type": format!("{:?}", a.kind), "damage": a.damage, "hits": a.hits}))
        .collect()
}
