//! `Byrdonis` (`Models.Monsters/Byrdonis.cs`): gains 1 Territorial when added to the room. Swoop
//! and Peck (3 hits), alternating, Swoop first.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const SWOOP: StateIdx = 0;
const PECK: StateIdx = 1;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "SWOOP_MOVE",
            follow_up: PECK,
        },
        StateDef::Move {
            id: "PECK_MOVE",
            follow_up: SWOOP,
        },
    ],
    initial: SWOOP,
};

/// `Byrdonis`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Byrdonis;

#[sts2_macros::hooks]
impl Hooks for Byrdonis {
    type Me = CreatureId;
}

impl MonsterModel for Byrdonis {
    fn name(&self) -> &'static str {
        "Byrdonis"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 90, 81)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 90, 84)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            SWOOP => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 19, 17)),
            PECK => attack_players_hits(
                eng,
                me,
                if_ascension(ascension, DEADLY_ENEMIES, 4, 3),
                // `PeckRepeat` is 3 at every ascension.
                3,
            ),
            _ => unreachable!("not a Byrdonis move: {state}"),
        }
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.apply_power(me, PowerKind::Territorial, Decimal::ONE, Some(me))
    }
}
