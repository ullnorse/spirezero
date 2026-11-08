//! `Inklet` (`Models.Monsters/Inklet.cs`): gains 1 Slippery when added to the room.
//!
//! Opens with Jab (Whirlwind for the middle Inklet); after Jab a random branch over Piercing
//! Gaze and Whirlwind (neither twice in a row), each of which goes back to Jab.

use super::machine::{Branch, MoveTable, Repeat, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const JAB: StateIdx = 0;
const PIERCING_GAZE: StateIdx = 1;
const WHIRLWIND: StateIdx = 2;
const RAND: StateIdx = 3;

/// `_whirlwindRepeat`.
const WHIRLWIND_REPEAT: u32 = 3;

/// The states, in `GenerateMoveStateMachine`'s list order; `INIT_RAND` is built but never
/// reachable, so it is left out.
const STATES: &[StateDef] = &[
    StateDef::Move {
        id: "JAB_MOVE",
        follow_up: RAND,
    },
    StateDef::Move {
        id: "PIERCING_GAZE_MOVE",
        follow_up: JAB,
    },
    StateDef::Move {
        id: "WHIRLWIND_MOVE",
        follow_up: JAB,
    },
    StateDef::Random {
        id: "RAND",
        branches: &[
            Branch::new(PIERCING_GAZE, Repeat::CannotRepeat),
            Branch::new(WHIRLWIND, Repeat::CannotRepeat),
        ],
    },
];

static MOVES: MoveTable = MoveTable {
    states: STATES,
    initial: JAB,
};

static MOVES_MIDDLE: MoveTable = MoveTable {
    states: STATES,
    initial: WHIRLWIND,
};

/// Per-combat fields, set by the encounter before `SetUpForCombat`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Inklet {
    /// `MiddleInklet`: opens with Whirlwind.
    pub middle_inklet: bool,
}

#[sts2_macros::hooks]
impl Hooks for Inklet {
    type Me = CreatureId;
}

impl MonsterModel for Inklet {
    fn name(&self) -> &'static str {
        "Inklet"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 12, 11)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 18, 17)
    }

    fn moves(&self) -> &'static MoveTable {
        if self.middle_inklet {
            &MOVES_MIDDLE
        } else {
            &MOVES
        }
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            JAB => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 4, 3)),
            PIERCING_GAZE => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 11, 10))
            }
            WHIRLWIND => attack_players_hits(
                eng,
                me,
                if_ascension(ascension, DEADLY_ENEMIES, 3, 2),
                WHIRLWIND_REPEAT,
            ),
            _ => unreachable!("not an Inklet move: {state}"),
        }
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.apply_power(me, PowerKind::Slippery, Decimal::ONE, Some(me))
    }
}
