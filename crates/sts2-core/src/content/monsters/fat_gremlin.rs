//! `FatGremlin` (`Models.Monsters/FatGremlin.cs`): a gremlin `SurprisePower` adds when the merc dies,
//! holding the gold it stole as `HeistPower`s. It spends its first turn spawning (a stun intent),
//! then flees: `CreatureCmd.Escape`, which drops its powers (the gold stays stolen) and counts
//! against the encounter's gold. Killing it first returns the gold. `IsAwake` is animation only.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{MonsterModel, TOUGH_ENEMIES, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};

const SPAWNED: StateIdx = 0;
const FLEE: StateIdx = 1;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "SPAWNED_MOVE",
            follow_up: FLEE,
        },
        StateDef::Move {
            id: "FLEE_MOVE",
            follow_up: FLEE,
        },
    ],
    initial: SPAWNED,
};

/// `FatGremlin`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FatGremlin;

#[sts2_macros::hooks]
impl Hooks for FatGremlin {
    type Me = CreatureId;
}

impl MonsterModel for FatGremlin {
    fn name(&self) -> &'static str {
        "Fat Gremlin"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 14, 13)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 18, 17)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        match state {
            SPAWNED => Ok(()),
            FLEE => {
                eng.escape(me);
                Ok(())
            }
            _ => unreachable!("not a FatGremlin move: {state}"),
        }
    }
}
