//! `Byrdpip` (`Models.Monsters/Byrdpip.cs`): the pet `Models.Relics/Byrdpip` adds each combat. 9999/9999
//! HP (a hidden health bar) and a single move that does nothing; it never takes a turn or rolls a
//! move (it is on the player side). The skin is cosmetic.

use super::MonsterModel;
use super::machine::{MoveTable, StateDef, StateIdx};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};

/// `MinInitialHp = MaxInitialHp = 9999`.
const INITIAL_HP: i32 = 9999;

static MOVES: MoveTable = MoveTable {
    // `moveState.FollowUpState = moveState`; a pet never rolls, so it is never followed.
    states: &[StateDef::Move {
        id: "NOTHING_MOVE",
        follow_up: 0,
    }],
    initial: 0,
};

/// `Byrdpip`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Byrdpip;

#[sts2_macros::hooks]
impl Hooks for Byrdpip {
    type Me = CreatureId;
}

impl MonsterModel for Byrdpip {
    fn name(&self) -> &'static str {
        "Byrdpip"
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
