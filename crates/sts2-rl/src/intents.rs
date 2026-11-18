//! The enemies' intents without a scratch copy per observation (`docs/specs/rl-2x-plan.md`,
//! section 7.3).
//!
//! Recomputing an enemy's intents runs its move on a copy of the state
//! ([`Hooked::intents_into`], on a scratch this cache owns and refills with `clone_from`). The
//! cache keeps each enemy's last intents, with each attack's base damage, under the key they
//! were computed with, and recomputes only when the key changes:
//!
//! - **Token key**, for a move whose intents are a function of the monster alone: the creature
//!   is in combat, its whole [`Monster`] (kind with its fields, the move machine, the RNG, the
//!   roll flags) and the ascension. The declared intents, status-card counts and each attack's
//!   base are a function of exactly these. An attack's damage is the base folded through the
//!   damage hooks, which is done live on every observation ([`Hooked::intent_damage`]).
//! - **Listener key**, for a move with a buff or debuff intent and no attack: the token key
//!   plus the listener generation ([`crate::listeners`]), the number of combat cards and the
//!   encounter's data. A buff or debuff intent's [`Intent::applies`] is what the move applied on
//!   the scratch copy, after the receiver's modifiers (listener state).
//! - **Full key**, for a move with an attack and a buff, debuff or status-card intent, or with
//!   more than one attack: the listener key plus every creature's HP, max HP and block. What
//!   follows an attack in the move happens only if the attack left the player alive (their HP
//!   and block).
//!
//! The forecast moves of `obs_v2` ([`IntentCache::forecast_intents`]) are cached the same way,
//! one entry per creature and forecast step, with the forecast move added to the key.
//!
//! **Oracle:** debug builds (and the `regress` profile) recompute every hit through the slow
//! path ([`CombatState::intents`]) and assert equality, so a move or modifier that reads state
//! outside the key fails at once (it then needs the full key, or the key grows).
//!
//! [`Monster`]: sts2_core::content::monsters::Monster

use sts2_core::Decimal;
use sts2_core::combat::{CombatState, CreatureId, Hooked, Intent, IntentScratch, IntentType};
use sts2_core::content::monsters::machine::StateIdx;

use crate::key::Words;
use crate::listeners::ListenerState;

/// Forecast moves cached per creature ([`IntentCache::forecast_intents`]).
pub const FORECAST_STEPS: usize = 4;

/// Which key an entry was made under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Key {
    Token,
    Listeners,
    Full,
}

#[derive(Clone, Debug, Default)]
struct Entry {
    /// `None` while empty.
    key: Option<Key>,
    words: Vec<u64>,
    intents: Vec<Intent>,
    /// The base damage of each intent with an attack, in order.
    bases: Vec<Decimal>,
}

/// Cached intents by [`CreatureId`].
#[derive(Debug, Default)]
pub struct IntentCache {
    entries: Vec<Entry>,
    /// The forecast moves' entries, [`FORECAST_STEPS`] per creature.
    forecast: Vec<Entry>,
    words: Vec<u64>,
    scratch: IntentScratch,
    listeners: ListenerState,
    /// The listener generation of the state being observed, once a full key needed it.
    generation: Option<u64>,
}

/// A copy of the cache (CB2: a search copy observes as its source would); the scratch
/// buffers are not part of its state and start empty.
impl Clone for IntentCache {
    fn clone(&self) -> Self {
        Self {
            entries: self.entries.clone(),
            forecast: self.forecast.clone(),
            words: self.words.clone(),
            scratch: IntentScratch::default(),
            listeners: self.listeners.clone(),
            generation: self.generation,
        }
    }
}

/// The key `intents` (with `bases`) need ([`Key`]).
fn key_of(intents: &[Intent], bases: &[Decimal]) -> Key {
    let has = |kinds: &[IntentType]| intents.iter().any(|i| kinds.contains(&i.kind));
    let applies = has(&[
        IntentType::Buff,
        IntentType::Debuff,
        IntentType::DebuffStrong,
    ]);
    if bases.len() > 1 || (!bases.is_empty() && (applies || has(&[IntentType::StatusCard]))) {
        Key::Full
    } else if applies {
        Key::Listeners
    } else {
        Key::Token
    }
}

/// Writes the token key of `enemy` into `words`.
fn token(words: &mut Vec<u64>, s: &CombatState, enemy: CreatureId) {
    words.clear();
    let mut w = Words(words);
    let present = s.contains_creature(enemy);
    w.add(&present);
    if present {
        w.add(&s.creature(enemy).monster);
    }
    w.add(&s.ascension);
}

/// Appends the rest of `key` to the token key in `words`.
fn rest(words: &mut Vec<u64>, s: &CombatState, generation: u64, key: Key) {
    if key == Key::Token {
        return;
    }
    let mut w = Words(words);
    w.add(&generation);
    w.add(&s.cards.len());
    w.add(&s.encounter_data);
    if key == Key::Full {
        w.add(&s.creatures.len());
        for c in &s.creatures {
            w.add(&(c.hp, c.max_hp, c.block));
        }
    }
}

impl IntentCache {
    /// Starts a new episode: no cached value of the last one is reused unless its whole key
    /// matches again.
    pub fn reset(&mut self) {
        self.listeners.reset();
    }

    /// Starts the observation of a new state.
    pub const fn begin(&mut self) {
        self.generation = None;
    }

    /// `enemy`'s intents on `h` (the state of the observation [`IntentCache::begin`] started).
    pub fn intents(&mut self, h: Hooked<'_>, enemy: CreatureId) -> &[Intent] {
        self.lookup(h, enemy, None)
    }

    /// The intents of `enemy`'s table move `at` as its `step`th forecast move
    /// ([`Hooked::intents_of_move_into`]) on `h`, cached as [`IntentCache::intents`] is, under
    /// the same keys with `at` added (`step` only picks the entry: below [`FORECAST_STEPS`]).
    ///
    /// # Panics
    /// If `step` is not below [`FORECAST_STEPS`].
    pub fn forecast_intents(
        &mut self,
        h: Hooked<'_>,
        enemy: CreatureId,
        step: usize,
        at: StateIdx,
    ) -> &[Intent] {
        assert!(step < FORECAST_STEPS, "forecast step {step}");
        self.lookup(h, enemy, Some((step, at)))
    }

    fn lookup(
        &mut self,
        h: Hooked<'_>,
        enemy: CreatureId,
        forecast: Option<(usize, StateIdx)>,
    ) -> &[Intent] {
        let Self {
            entries,
            forecast: forecast_entries,
            words,
            scratch,
            listeners,
            generation,
        } = self;
        let s = h.s;
        let (entries, index) = match forecast {
            None => (entries, enemy.0 as usize),
            Some((step, _)) => (forecast_entries, enemy.0 as usize * FORECAST_STEPS + step),
        };
        if entries.len() <= index {
            entries.resize_with(index + 1, Entry::default);
        }
        token(words, s, enemy);
        if let Some((_, at)) = forecast {
            Words(words).add(&at);
        }
        let token_len = words.len();
        let entry = &mut entries[index];
        let hit = match entry.key {
            None => false,
            Some(Key::Token) => entry.words == *words,
            Some(key) => {
                let generation = *generation.get_or_insert_with(|| listeners.update(s));
                rest(words, s, generation, key);
                entry.words == *words
            }
        };
        if hit {
            if entry.key == Some(Key::Token) {
                let mut bases = entry.bases.iter();
                for attack in entry.intents.iter_mut().filter_map(|i| i.attack.as_mut()) {
                    if let Some(&base) = bases.next() {
                        attack.damage = h.intent_damage(enemy, base);
                    }
                }
            }
            #[cfg(debug_assertions)]
            match forecast {
                None => debug_assert_eq!(
                    entry.intents,
                    s.intents(enemy),
                    "cached intents of {enemy:?} are stale: the move or a modifier reads state outside the key",
                ),
                Some((_, at)) => {
                    let (mut fresh, mut bases) = (Vec::new(), Vec::new());
                    h.intents_of_move_into(enemy, at, scratch, &mut fresh, &mut bases);
                    debug_assert_eq!(
                        entry.intents, fresh,
                        "cached forecast intents of {enemy:?} at move {at} are stale",
                    );
                }
            }
            return &entry.intents;
        }
        match forecast {
            None => h.intents_into(enemy, scratch, &mut entry.intents, &mut entry.bases),
            Some((_, at)) => {
                h.intents_of_move_into(enemy, at, scratch, &mut entry.intents, &mut entry.bases);
            }
        }
        let key = key_of(&entry.intents, &entry.bases);
        words.truncate(token_len);
        if key != Key::Token {
            let generation = *generation.get_or_insert_with(|| listeners.update(s));
            rest(words, s, generation, key);
        }
        entry.key = Some(key);
        std::mem::swap(&mut entry.words, words);
        &entry.intents
    }
}
