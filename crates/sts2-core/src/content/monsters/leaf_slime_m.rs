//! `LeafSlimeM` (`Models.Monsters/LeafSlimeM.cs`): Sticky Shot (2 Slimed into the discard pile)
//! first, then Clump Shot and Sticky Shot alternating. No random branches.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};

const CLUMP_SHOT: StateIdx = 0;
const STICKY_SHOT: StateIdx = 1;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "CLUMP_SHOT",
            follow_up: STICKY_SHOT,
        },
        StateDef::Move {
            id: "STICKY_SHOT",
            follow_up: CLUMP_SHOT,
        },
    ],
    initial: STICKY_SHOT,
};

/// `LeafSlimeM`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LeafSlimeM;

#[sts2_macros::hooks]
impl Hooks for LeafSlimeM {
    type Me = CreatureId;
}

impl MonsterModel for LeafSlimeM {
    fn name(&self) -> &'static str {
        "Leaf Slime (M)"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 33, 32)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 36, 35)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        match state {
            CLUMP_SHOT => {
                let damage = if_ascension(eng.s.ascension, DEADLY_ENEMIES, 9, 8);
                attack_players(eng, me, damage)
            }
            STICKY_SHOT => super::add_slimed(eng, 2),
            _ => unreachable!("not a LeafSlimeM move: {state}"),
        }
    }
}
