//! `SneakyGremlin` (`Models.Monsters/SneakyGremlin.cs`): a gremlin `SurprisePower` adds when the merc
//! dies. It spends its first turn spawning (a stun intent), then Tackles every turn. `IsAwake` is
//! animation only.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};

const SPAWNED: StateIdx = 0;
const TACKLE: StateIdx = 1;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "SPAWNED_MOVE",
            follow_up: TACKLE,
        },
        StateDef::Move {
            id: "TACKLE_MOVE",
            follow_up: TACKLE,
        },
    ],
    initial: SPAWNED,
};

/// `SneakyGremlin`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SneakyGremlin;

#[sts2_macros::hooks]
impl Hooks for SneakyGremlin {
    type Me = CreatureId;
}

impl MonsterModel for SneakyGremlin {
    fn name(&self) -> &'static str {
        "Sneaky Gremlin"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 11, 10)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 15, 14)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        match state {
            SPAWNED => Ok(()),
            TACKLE => attack_players(
                eng,
                me,
                if_ascension(eng.s.ascension, DEADLY_ENEMIES, 10, 9),
            ),
            _ => unreachable!("not a SneakyGremlin move: {state}"),
        }
    }
}
