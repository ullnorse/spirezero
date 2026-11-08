//! `TrackerRubyRaider` (`Models.Monsters/TrackerRubyRaider.cs`): Track (2 Frail) once, then
//! Hounds (many 1-damage hits) every turn.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players_hits, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const TRACK: StateIdx = 0;
const HOUNDS: StateIdx = 1;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "TRACK_MOVE",
            follow_up: HOUNDS,
        },
        StateDef::Move {
            id: "HOUNDS_MOVE",
            follow_up: HOUNDS,
        },
    ],
    initial: TRACK,
};

/// `TrackerRubyRaider`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TrackerRubyRaider;

#[sts2_macros::hooks]
impl Hooks for TrackerRubyRaider {
    type Me = CreatureId;
}

impl MonsterModel for TrackerRubyRaider {
    fn name(&self) -> &'static str {
        "Tracker Raider"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 22, 21)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 26, 25)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            TRACK => {
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, PowerKind::Frail, Decimal::from(2), Some(me))
            }
            HOUNDS => {
                let hits = if_ascension(ascension, DEADLY_ENEMIES, 9, 8).cast_unsigned();
                attack_players_hits(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 1, 1), hits)
            }
            _ => unreachable!("not a TrackerRubyRaider move: {state}"),
        }
    }
}
