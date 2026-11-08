//! `SewerClam` (`Models.Monsters/SewerClam.cs`): Jet <-> Pressurize (4 Strength), opening with Jet.
//! It enters with 8 Plating (9 from `ToughEnemies`).

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const PRESSURIZE: StateIdx = 0;
const JET: StateIdx = 1;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "PRESSURIZE_MOVE",
            follow_up: JET,
        },
        StateDef::Move {
            id: "JET_MOVE",
            follow_up: PRESSURIZE,
        },
    ],
    initial: JET,
};

/// `SewerClam`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SewerClam;

#[sts2_macros::hooks]
impl Hooks for SewerClam {
    type Me = CreatureId;
}

impl MonsterModel for SewerClam {
    fn name(&self) -> &'static str {
        "Sewer Clam"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 58, 56)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            PRESSURIZE => eng.apply_power(me, PowerKind::Strength, Decimal::from(4), Some(me)),
            JET => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 11, 10)),
            _ => unreachable!("not a SewerClam move: {state}"),
        }
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        let amount = if_ascension(eng.s.ascension, TOUGH_ENEMIES, 9, 8);
        eng.apply_power(me, PowerKind::Plating, Decimal::from(amount), Some(me))
    }
}
