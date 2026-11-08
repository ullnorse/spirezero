//! `DevotedSculptor` (`Models.Monsters/DevotedSculptor.cs`): Forbidden Incantation (9 Ritual),
//! then Savage every turn.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const FORBIDDEN_INCANTATION: StateIdx = 0;
const SAVAGE: StateIdx = 1;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "FORBIDDEN_INCANTATION_MOVE",
            follow_up: SAVAGE,
        },
        StateDef::Move {
            id: "SAVAGE_MOVE",
            follow_up: SAVAGE,
        },
    ],
    initial: FORBIDDEN_INCANTATION,
};

/// `_ritualGain`.
const RITUAL_GAIN: i32 = 9;

/// `DevotedSculptor`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DevotedSculptor;

#[sts2_macros::hooks]
impl Hooks for DevotedSculptor {
    type Me = CreatureId;
}

impl MonsterModel for DevotedSculptor {
    fn name(&self) -> &'static str {
        "Devoted Sculptor"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 172, 162)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        match state {
            FORBIDDEN_INCANTATION => {
                eng.apply_power(me, PowerKind::Ritual, Decimal::from(RITUAL_GAIN), None)
            }
            SAVAGE => attack_players(
                eng,
                me,
                if_ascension(eng.s.ascension, DEADLY_ENEMIES, 15, 12),
            ),
            _ => unreachable!("not a DevotedSculptor move: {state}"),
        }
    }
}
