//! The privileged buffer of the run env (`docs/plans/necro-e2e.md`, WP2).
//!
//! What the player cannot see, for the value head only. It is written only with [`crate::RunOptions::priv_obs`],
//! never read by the observation ([`crate::run_obs`] does not import this module), and a
//! policy must not see it (`tests/leak.rs`, H1: the hidden-state perturbations leave the
//! observation unchanged and change these bytes).
//!
//! Little-endian plain integers like the observation; content ids are [`crate::vocab`] ids (0 an
//! empty slot), move ids are [`move_id`]. [`PRIV_BYTES`] in all, zero where nothing applies:
//! - [`HEADER_AT`] (8 bytes): in a fight, draw pile size, 1 when a next act exists, 0 × 5.
//! - [`DRAW_AT`]: the true top of the draw pile, [`DRAW`] × (card `u16`, upgrade `u8`), the top
//!   first.
//! - [`MOVES_AT`]: [`CREATURES`] × (the move shown now `u16`, the move after it `u16`), in the
//!   observation's creature slots (allies, then enemies); 0 for a slot without a monster. The
//!   move after is a peek ([`sts2_core::content::monsters::following_moves`]).
//! - [`POINTS_AT`]: [`MAP_POINTS`] × room type `u8` (`RoomType::enum_index`, 0 none), in the
//!   observation's map order (start, points, boss, second boss): a fixed point's type, and for
//!   a `?` that has not been entered what it becomes if entered next
//!   ([`sts2_core::run::RunState::peek_unknown_room`]).
//! - [`NORMALS_AT`], [`ELITES_AT`], [`EVENTS_AT`]: the act's next [`NORMALS`] normal and
//!   [`ELITES`] elite encounters and next [`EVENTS`] events (`u16` ids), in the order they will
//!   be pulled.
//! - [`NEXT_AT`]: the next act's boss and second boss (`u16`), then its map as the observation
//!   writes one ([`MAP_POINTS`] × (column, row, type + 1, 0, children bitmask)); zero in the
//!   last act.

use sts2_core::combat::CombatState;
use sts2_core::content::monsters;
use sts2_core::rng::RunRngSet;
use sts2_core::run::odds::UnknownMapPointOdds;
use sts2_core::run::{MapPoint, Run, RunState};

use crate::obs::Writer;
use crate::run_obs::{MAP_POINT_BYTES, MAP_POINTS};
use crate::vocab::{self, FNV_BASIS, fnv};

/// Draw pile cards.
pub const DRAW: usize = 10;
/// Creature slots (as the observation's).
pub const CREATURES: usize = crate::obs::CREATURES;
/// Upcoming normal encounters, elite encounters and events.
pub const NORMALS: usize = 8;
pub const ELITES: usize = 4;
pub const EVENTS: usize = 8;

pub const HEADER_AT: usize = 0;
pub const DRAW_AT: usize = 8;
pub const MOVES_AT: usize = DRAW_AT + DRAW * 3;
pub const POINTS_AT: usize = MOVES_AT + CREATURES * 4;
pub const NORMALS_AT: usize = POINTS_AT + MAP_POINTS;
pub const ELITES_AT: usize = NORMALS_AT + NORMALS * 2;
pub const EVENTS_AT: usize = ELITES_AT + ELITES * 2;
pub const NEXT_AT: usize = EVENTS_AT + EVENTS * 2;
/// Bytes of the privileged buffer.
pub const PRIV_BYTES: usize = NEXT_AT + 4 + MAP_POINTS * MAP_POINT_BYTES;

/// A move's `MoveState.Id` as a 16-bit id: FNV-1a folded, never 0.
#[must_use]
pub fn move_id(id: &str) -> u16 {
    let h = fnv(FNV_BASIS, id.as_bytes());
    let folded = (h ^ (h >> 16) ^ (h >> 32) ^ (h >> 48)).to_le_bytes();
    u16::from_le_bytes([folded[0], folded[1]]).max(1)
}

/// What the rooms and next-act sections ([`POINTS_AT`] on) depend on: they are rewritten only
/// when it changes (a `?` is entered, a room is pulled, a relic is obtained, the act changes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RoomsKey {
    act: usize,
    visited: usize,
    history: usize,
    relics: usize,
    odds: UnknownMapPointOdds,
    pulled: [usize; 3],
}

/// One env's writer of the privileged buffer, with the rooms sections it computed.
#[derive(Clone, Debug, Default)]
pub struct PrivObserver {
    key: Option<RoomsKey>,
    /// The next act's map, which only depends on the seed and the act.
    next: Option<(usize, Vec<u8>)>,
    rooms: Vec<u8>,
    moves: Vec<Option<&'static str>>,
}

impl PrivObserver {
    /// Starts a new episode.
    pub fn reset(&mut self) {
        self.key = None;
        self.next = None;
    }

    /// Writes the privileged buffer of `run` into `out` ([`PRIV_BYTES`]).
    ///
    /// # Panics
    /// If `out` is shorter than [`PRIV_BYTES`].
    pub fn write(&mut self, run: &Run, out: &mut [u8]) {
        let combat = run.combat().map(sts2_core::Combat::state);
        self.write_parts(run.state(), run.rng(), combat, out);
    }

    /// [`PrivObserver::write`] of a run with state `s`, streams `rng` ([`Run::rng`]) and the
    /// fight `combat`, so a test can write a state it changed.
    ///
    /// # Panics
    /// If `out` is shorter than [`PRIV_BYTES`].
    pub fn write_parts(
        &mut self,
        s: &RunState,
        rng: &RunRngSet,
        combat: Option<&CombatState>,
        out: &mut [u8],
    ) {
        let mut w = Writer::new(&mut out[..PRIV_BYTES]);
        let next_act = s.act_index + 1 < s.acts.len();
        w.u8(u8::from(combat.is_some()));
        w.u8(combat.map_or(0, |c| c.player.draw.len()));
        w.u8(u8::from(next_act));
        if let Some(c) = combat {
            w.seek(DRAW_AT);
            for &id in c.player.draw.iter().take(DRAW) {
                let card = c.card(id);
                w.u16(vocab::card(card.kind));
                w.u8(card.upgrade_level);
            }
            monsters::following_moves(c, &mut self.moves);
            w.seek(MOVES_AT);
            for &id in c.allies.iter().chain(&c.enemies).take(CREATURES) {
                let creature = c.creature(id);
                if let Some(m) = &creature.monster {
                    w.u16(move_id(m.next_move_name()));
                    w.u16(self.moves[id.index()].map_or(0, move_id));
                } else {
                    w.skip(4);
                }
            }
        }
        let key = RoomsKey {
            act: s.act_index,
            visited: s.visited.len(),
            history: s.history.len(),
            relics: s.relics.len(),
            odds: s.unknown_odds,
            pulled: [
                s.act().normal_encounters_visited,
                s.act().elite_encounters_visited,
                s.act().events_visited,
            ],
        };
        if self.key.as_ref() != Some(&key) {
            self.rooms(s, rng);
            self.key = Some(key);
        }
        out[POINTS_AT..PRIV_BYTES].copy_from_slice(&self.rooms[POINTS_AT..]);
    }

    /// Writes the rooms and next-act sections into `self.rooms` (at their offsets).
    fn rooms(&mut self, s: &RunState, rng: &RunRngSet) {
        self.rooms.resize(PRIV_BYTES, 0);
        let mut w = Writer::new(&mut self.rooms);
        w.seek(POINTS_AT);
        let m = &s.map;
        let points = std::iter::once(m.start())
            .chain(m.points())
            .chain(std::iter::once(m.boss()))
            .chain(m.second_boss());
        for p in points.take(MAP_POINTS) {
            w.u8(room_of(s, rng, p).map_or(0, sts2_core::run::RoomType::enum_index));
        }
        let act = s.act();
        w.seek(NORMALS_AT);
        for k in 0..NORMALS {
            let list = &act.normal_encounters;
            w.u16(vocab::encounter_id(
                list[(act.normal_encounters_visited + k) % list.len()].id,
            ));
        }
        for k in 0..ELITES {
            let list = &act.elite_encounters;
            w.u16(vocab::encounter_id(
                list[(act.elite_encounters_visited + k) % list.len()].id,
            ));
        }
        for k in 0..EVENTS {
            let id = act.events.get(act.events_visited + k);
            w.u16(id.map_or(vocab::PAD, |id| vocab::event_id(id)));
        }
        let next = s.act_index + 1;
        if next < s.acts.len() {
            w.u16(vocab::encounter_id(s.acts[next].boss.id));
            w.u16(
                s.acts[next]
                    .second_boss
                    .map_or(vocab::PAD, |b| vocab::encounter_id(b.id)),
            );
            if self.next.as_ref().is_none_or(|(act, _)| *act != next) {
                let mut bytes = vec![0; MAP_POINTS * MAP_POINT_BYTES];
                if let Some(map) = s.peek_next_act_map(rng) {
                    let mut mw = Writer::new(&mut bytes);
                    let points = std::iter::once(map.start())
                        .chain(map.points())
                        .chain(std::iter::once(map.boss()))
                        .chain(map.second_boss());
                    for p in points.take(MAP_POINTS) {
                        mw.u8(p.coord.col);
                        mw.u8(p.coord.row);
                        mw.u8(p.kind as u8 + 1);
                        mw.u8(0);
                        mw.u8(children_mask(p));
                    }
                }
                self.next = Some((next, bytes));
            }
            if let Some((_, bytes)) = &self.next {
                self.rooms[NEXT_AT + 4..PRIV_BYTES].copy_from_slice(bytes);
            }
        }
    }
}

fn children_mask(p: &MapPoint) -> u8 {
    p.children.iter().fold(0, |bits, c| {
        bits | u32::try_from(c.col)
            .ok()
            .and_then(|col| 1_u8.checked_shl(col))
            .unwrap_or(0)
    })
}

/// The room point `p` becomes: its fixed type, or for an unvisited `?` the peeked roll.
fn room_of(s: &RunState, rng: &RunRngSet, p: &MapPoint) -> Option<sts2_core::run::RoomType> {
    sts2_core::run::RoomType::for_point(p.kind).or_else(|| {
        (!s.visited.contains(&p.coord))
            .then(|| s.peek_unknown_room(rng, p.coord))
            .flatten()
    })
}
