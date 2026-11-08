//! `LeafSlimeS` (`Models.Monsters/LeafSlimeS.cs`): starts at a random branch between Tackle and
//! Goop (1 Slimed into the discard pile), neither repeating, and returns to it after each move.

use super::machine::{Branch, MoveTable, Repeat, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};

const TACKLE: StateIdx = 0;
const GOOP: StateIdx = 1;
const RAND: StateIdx = 2;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "TACKLE_MOVE",
            follow_up: RAND,
        },
        StateDef::Move {
            id: "GOOP_MOVE",
            follow_up: RAND,
        },
        StateDef::Random {
            id: "RAND",
            branches: &[
                Branch::new(TACKLE, Repeat::CannotRepeat),
                Branch::new(GOOP, Repeat::CannotRepeat),
            ],
        },
    ],
    initial: RAND,
};

/// `LeafSlimeS`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LeafSlimeS;

#[sts2_macros::hooks]
impl Hooks for LeafSlimeS {
    type Me = CreatureId;
}

impl MonsterModel for LeafSlimeS {
    fn name(&self) -> &'static str {
        "Leaf Slime (S)"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 12, 11)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 16, 15)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        match state {
            TACKLE => {
                let damage = if_ascension(eng.s.ascension, DEADLY_ENEMIES, 4, 3);
                attack_players(eng, me, damage)
            }
            GOOP => super::add_slimed(eng, 1),
            _ => unreachable!("not a LeafSlimeS move: {state}"),
        }
    }
}
