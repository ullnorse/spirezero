//! The run observation's full-view section (RLN30, `RunOptions::obs_full`): what a player sees
//! that the v1 and v2 bytes leave out. Today that is each enemy's past moves.
//!
//! Written at [`FULL_AT`] (after the v2 section, which does not move) only with `obs_full`, during
//! a fight; zero otherwise. [`FULL_BYTES`] in all:
//!
//! - [`HISTORY_AT`]: per creature slot ([`CREATURES`], the combat section's order; enemies only,
//!   allies stay zero), [`HISTORY_SLOT_BYTES`]: how many past moves the engine logged (`u8`,
//!   saturating), then its last [`HISTORY`] past moves, newest first, each [`MOVE_BYTES`]: the
//!   move's key (`u16`, [`move_key`], 0 for none), whether its intent was seen (`u8`: 1 when
//!   this observer saw the move as the enemy's intent, else 0 and the rest zero), then as it was
//!   last shown: its first intent's kind + 1 (`u8`), damage per hit and hits (`i16`, `u8`; 0 for
//!   a non-attack) and the move's whole attack (`i16`).
//!
//! **What a past move is.** The engine keeps each monster's `StateLog` ([`MoveLog`]): every move
//! its machine entered, which is every intent it showed after a roll. The last entry is the move
//! it shows now (its next move, unless a forced move replaced it), so it is left out
//! ([`past_moves`]); what is left was shown earlier and is never a move to come: the next roll
//! logs its move again. A forced move (stun, revive...) is not logged, and a logged move a
//! forced move replaced stays in the log though it was not performed. The log keeps its last
//! [`MoveLog::TAIL`] entries, so at most that many moves show (one fewer unless a forced move
//! holds the enemy).
//!
//! **How it showed.** A past move is not re-run on the current state (its effects may read state
//! that has moved on): its intent bytes are the ones the observer saw while it was the enemy's
//! move, at the last observation before the next roll ([`Shown`], keyed by the move's place in
//! the log). A move entered and left between two observations (a turn decided automatically) or
//! before the observer started (a fight joined mid-way) has its key but no intent bytes.
//!
//! [`MoveLog`]: sts2_core::content::monsters::machine::MoveLog

use sts2_core::Combat;
use sts2_core::combat::{Intent, IntentType};
use sts2_core::content::monsters::Monster;
use sts2_core::content::monsters::machine::{MoveLog, StateIdx};

use crate::intents::IntentCache;
use crate::obs::{CREATURES, Writer};
use crate::obs_v2::{V2_AT, V2_BYTES};
use crate::vocab::{FNV_BASIS, fnv};

/// Past moves per creature.
pub const HISTORY: usize = MoveLog::TAIL;
/// Bytes per past move.
pub const MOVE_BYTES: usize = 9;
/// Bytes of a move's intent as shown (kind, damage, hits, whole attack).
pub const INTENT_BYTES: usize = 6;
/// Bytes per creature: the count, then the moves.
pub const HISTORY_SLOT_BYTES: usize = 1 + HISTORY * MOVE_BYTES;

/// Offsets inside the section.
pub const HISTORY_AT: usize = 0;
/// Bytes of the section.
pub const FULL_BYTES: usize = HISTORY_AT + CREATURES * HISTORY_SLOT_BYTES;

/// The section's offset in the run observation.
pub const FULL_AT: usize = V2_AT + V2_BYTES;

/// A move's id as a 16-bit key: FNV-1a folded, never 0.
#[must_use]
pub fn move_key(id: &str) -> u16 {
    let h = fnv(FNV_BASIS, id.as_bytes());
    let folded = (h ^ (h >> 16) ^ (h >> 32) ^ (h >> 48)).to_le_bytes();
    u16::from_le_bytes([folded[0], folded[1]]).max(1)
}

/// The moves `m` showed before the one it shows now, newest first (module doc), as (place in
/// the log, move), and how many the log holds in all.
pub fn past_moves(m: &Monster) -> (usize, impl Iterator<Item = (usize, StateIdx)> + '_) {
    let log = &m.machine.log;
    let now = usize::from(now_logged(m));
    let len = log.len();
    let kept = len.min(MoveLog::TAIL);
    let moves = log
        .recent(kept)
        .enumerate()
        .skip(now)
        .map(move |(back, state)| (len - 1 - back, state));
    (len - now, moves)
}

/// Whether the move `m` shows now is its log's last entry (not a forced move).
fn now_logged(m: &Monster) -> bool {
    m.machine.forced.is_none() && m.machine.log.last() == Some(m.machine.current)
}

/// A move's intent as shown: its first intent's kind + 1, damage per hit, hits, whole attack.
fn intent_bytes(intents: &[Intent]) -> [u8; INTENT_BYTES] {
    let head = intents.first();
    let attack = head.and_then(|i| i.attack);
    let total = intents
        .iter()
        .filter(|i| matches!(i.kind, IntentType::Attack | IntentType::DeathBlow))
        .filter_map(|i| i.attack)
        .map(|a| i64::from(a.damage) * i64::from(a.hits))
        .sum::<i64>();
    let mut out = [0; INTENT_BYTES];
    let mut w = Writer::new(&mut out);
    w.u8(head.map_or(0, |i| i.kind as u8 + 1));
    w.i16(attack.map_or(0, |a| a.damage));
    w.u8(attack.map_or(0, |a| a.hits));
    w.i16(i32::try_from(total).unwrap_or(i32::MAX));
    out
}

/// Records kept per creature: the moves [`HISTORY`] can show and the one shown now.
const KEPT: usize = HISTORY + 1;

/// What each enemy's moves showed, by creature index and place in the log (module doc). One
/// fight's; [`Shown::reset`] at a new one.
#[derive(Clone, Debug, Default)]
pub struct Shown {
    /// Per creature index: up to [`KEPT`] (place in the log, intent bytes), any order.
    by_creature: Vec<Vec<(usize, [u8; INTENT_BYTES])>>,
}

impl Shown {
    /// Forgets every record (a new fight or episode).
    pub fn reset(&mut self) {
        self.by_creature.iter_mut().for_each(Vec::clear);
    }

    /// The intent bytes the move at `place` of creature `index`'s log showed, if seen.
    fn get(&self, index: usize, place: usize) -> Option<&[u8; INTENT_BYTES]> {
        self.by_creature
            .get(index)?
            .iter()
            .find(|&&(p, _)| p == place)
            .map(|(_, bytes)| bytes)
    }

    /// Records that the move at `place` of creature `index`'s log shows `bytes` now.
    fn put(&mut self, index: usize, place: usize, bytes: [u8; INTENT_BYTES]) {
        if self.by_creature.len() <= index {
            self.by_creature.resize_with(index + 1, Vec::new);
        }
        let records = &mut self.by_creature[index];
        if let Some(r) = records.iter_mut().find(|r| r.0 == place) {
            r.1 = bytes;
            return;
        }
        if records.len() == KEPT {
            // The oldest place is never shown again.
            let oldest = records
                .iter()
                .enumerate()
                .min_by_key(|(_, r)| r.0)
                .map_or(0, |(i, _)| i);
            records.swap_remove(oldest);
        }
        records.push((place, bytes));
    }

    /// Writes the section of `combat` into `out` ([`FULL_BYTES`], zeroed first), then records
    /// what each enemy's move shows now (its intents through `cache`, the v2 observer's, on the
    /// state it [`IntentCache::begin`] started).
    ///
    /// # Panics
    /// If `out` is shorter than [`FULL_BYTES`].
    pub(crate) fn write(&mut self, cache: &mut IntentCache, combat: &Combat, out: &mut [u8]) {
        let hooked = combat.hooked();
        let state = hooked.s;
        let out = &mut out[..FULL_BYTES];
        out.fill(0);
        let mut w = Writer::new(out);
        let slots = state.allies.iter().chain(&state.enemies).copied();
        for (slot, id) in slots.take(CREATURES).enumerate() {
            let creature = state.creature(id);
            let Some(monster) = creature.monster.as_ref().filter(|_| creature.is_alive()) else {
                continue;
            };
            if !state.enemies.contains(&id) {
                continue;
            }
            let index = id.0 as usize;
            w.seek(HISTORY_AT + slot * HISTORY_SLOT_BYTES);
            let (count, moves) = past_moves(monster);
            w.u8(u8::try_from(count).unwrap_or(u8::MAX));
            let table = monster.kind.moves();
            for (place, move_idx) in moves {
                w.u16(move_key(table.states[move_idx].id()));
                match self.get(index, place) {
                    Some(bytes) => {
                        w.u8(1);
                        for &byte in bytes {
                            w.u8(byte);
                        }
                    }
                    None => w.skip(1 + INTENT_BYTES),
                }
            }
            if now_logged(monster) {
                let bytes = intent_bytes(cache.intents(hooked, id));
                self.put(index, monster.machine.log.len() - 1, bytes);
            }
        }
    }
}
