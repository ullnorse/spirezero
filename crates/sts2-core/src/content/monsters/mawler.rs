//! `Mawler` (`Models.Monsters/Mawler.cs`): opens with Claw, then a random branch over Rip and
//! Tear (never twice in a row), Roar (once; 3 Vulnerable) and Claw (never twice in a row).

use super::machine::{Branch, MoveTable, Repeat, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const RIP_AND_TEAR: StateIdx = 0;
const ROAR: StateIdx = 1;
const CLAW: StateIdx = 2;
const RAND: StateIdx = 3;

/// `_clawRepeat`.
const CLAW_REPEAT: u32 = 2;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "RIP_AND_TEAR_MOVE",
            follow_up: RAND,
        },
        StateDef::Move {
            id: "ROAR_MOVE",
            follow_up: RAND,
        },
        StateDef::Move {
            id: "CLAW_MOVE",
            follow_up: RAND,
        },
        StateDef::Random {
            id: "RAND",
            branches: &[
                Branch::new(RIP_AND_TEAR, Repeat::CannotRepeat),
                Branch::new(ROAR, Repeat::UseOnlyOnce),
                Branch::new(CLAW, Repeat::CannotRepeat),
            ],
        },
    ],
    initial: CLAW,
};

/// `Mawler`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Mawler;

#[sts2_macros::hooks]
impl Hooks for Mawler {
    type Me = CreatureId;
}

impl MonsterModel for Mawler {
    fn name(&self) -> &'static str {
        "Mawler"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 76, 72)
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
            RIP_AND_TEAR => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 16, 14))
            }
            ROAR => {
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, PowerKind::Vulnerable, Decimal::from(3), Some(me))
            }
            CLAW => attack_players_hits(
                eng,
                me,
                if_ascension(ascension, DEADLY_ENEMIES, 5, 4),
                CLAW_REPEAT,
            ),
            _ => unreachable!("not a Mawler move: {state}"),
        }
    }
}
