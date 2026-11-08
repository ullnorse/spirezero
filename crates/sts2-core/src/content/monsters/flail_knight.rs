//! `FlailKnight` (`Models.Monsters/FlailKnight.cs`): opens with Ram, then a random branch over War
//! Chant (+3 Strength, not twice in a row), Flail (2 hits) and Ram (both at most twice in a row),
//! each going back to the branch. `MysteriousKnight` is the same monster with extra starting powers.

use super::machine::{Branch, MoveTable, Repeat, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const WAR_CHANT: StateIdx = 0;
const FLAIL: StateIdx = 1;
const RAM: StateIdx = 2;
const RAND: StateIdx = 3;

/// `_flailRepeat`.
const FLAIL_REPEAT: u32 = 2;

pub(super) static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "WAR_CHANT",
            follow_up: RAND,
        },
        StateDef::Move {
            id: "FLAIL_MOVE",
            follow_up: RAND,
        },
        StateDef::Move {
            id: "RAM_MOVE",
            follow_up: RAND,
        },
        StateDef::Random {
            id: "RAND",
            branches: &[
                Branch::new(WAR_CHANT, Repeat::CannotRepeat),
                // `AddBranch(state, 2)`: `CanRepeatXTimes` with `maxTimes` 2.
                Branch::new(FLAIL, Repeat::CanRepeatXTimes(2)),
                Branch::new(RAM, Repeat::CanRepeatXTimes(2)),
            ],
        },
    ],
    initial: RAM,
};

/// `FlailKnight`'s hit points.
pub(super) const fn initial_hp(ascension: u8) -> i32 {
    if_ascension(ascension, TOUGH_ENEMIES, 108, 101)
}

/// `FlailKnight`'s moves, shared with `MysteriousKnight`.
pub(super) fn perform(eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
    let ascension = eng.s.ascension;
    match state {
        WAR_CHANT => eng.apply_power(me, PowerKind::Strength, Decimal::from(3), Some(me)),
        FLAIL => attack_players_hits(
            eng,
            me,
            if_ascension(ascension, DEADLY_ENEMIES, 10, 9),
            FLAIL_REPEAT,
        ),
        RAM => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 17, 15)),
        _ => unreachable!("not a FlailKnight move: {state}"),
    }
}

/// `FlailKnight`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FlailKnight;

#[sts2_macros::hooks]
impl Hooks for FlailKnight {
    type Me = CreatureId;
}

impl MonsterModel for FlailKnight {
    fn name(&self) -> &'static str {
        "Flail Knight"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        initial_hp(ascension)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        perform(eng, me, state)
    }
}
