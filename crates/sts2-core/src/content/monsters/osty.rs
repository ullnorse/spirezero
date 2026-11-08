//! `Osty` (`Models.Monsters/Osty.cs`): the Necrobinder's pet. 1/1 HP at creation and a single
//! move that does nothing; it never takes a turn or rolls a move (it is on the player side).

use super::MonsterModel;
use super::machine::{MoveTable, StateDef, StateIdx};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};

/// `MinInitialHp = MaxInitialHp = 1`.
const INITIAL_HP: i32 = 1;

static MOVES: MoveTable = MoveTable {
    // `FollowUpState` is unset in the game; Osty never rolls, so it is never followed.
    states: &[StateDef::Move {
        id: "NOTHING_MOVE",
        follow_up: 0,
    }],
    initial: 0,
};

/// `Osty`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Osty;

#[sts2_macros::hooks]
impl Hooks for Osty {
    type Me = CreatureId;
}

impl MonsterModel for Osty {
    fn name(&self) -> &'static str {
        "Osty"
    }

    fn min_initial_hp(&self, _ascension: u8) -> i32 {
        INITIAL_HP
    }

    fn max_initial_hp(&self, _ascension: u8) -> i32 {
        INITIAL_HP
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, _eng: &mut Engine<'_>, _me: CreatureId, _state: StateIdx) -> Flow {
        Ok(())
    }
}
