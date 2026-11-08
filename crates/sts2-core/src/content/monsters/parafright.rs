//! `Parafright` (`Models.Monsters/Parafright.cs`): The Obscura's illusion, 21 HP. Gains
//! `IllusionPower` (and with it `MinionPower`) when added to the room, then repeats Slam.
//! `ShouldDisappearFromDoom` is visual.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const SLAM: StateIdx = 0;

static MOVES: MoveTable = MoveTable {
    states: &[StateDef::Move {
        id: "SLAM_MOVE",
        follow_up: SLAM,
    }],
    initial: SLAM,
};

/// `Parafright`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Parafright;

#[sts2_macros::hooks]
impl Hooks for Parafright {
    type Me = CreatureId;
}

impl MonsterModel for Parafright {
    fn name(&self) -> &'static str {
        "Parafright"
    }

    fn min_initial_hp(&self, _ascension: u8) -> i32 {
        21
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.apply_power(me, PowerKind::Illusion, Decimal::ONE, Some(me))
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        assert_eq!(state, SLAM, "not a Parafright move: {state}");
        attack_players(
            eng,
            me,
            if_ascension(eng.s.ascension, DEADLY_ENEMIES, 17, 16),
        )
    }
}
