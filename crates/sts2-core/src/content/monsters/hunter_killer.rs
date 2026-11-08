//! `HunterKiller` (`Models.Monsters/HunterKiller.cs`): Tenderizing Goop (1 `Tender` on the
//! players) -> a random branch over Bite (not twice in a row) and Puncture (3 hits, at most twice
//! in a row), each going back to the branch.

use super::machine::{Branch, MoveTable, Repeat, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const GOOP: StateIdx = 0;
const BITE: StateIdx = 1;
const PUNCTURE: StateIdx = 2;
const RAND: StateIdx = 3;

/// `_punctureRepeat`.
const PUNCTURE_REPEAT: u32 = 3;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "TENDERIZING_GOOP_MOVE",
            follow_up: RAND,
        },
        StateDef::Move {
            id: "BITE_MOVE",
            follow_up: RAND,
        },
        StateDef::Move {
            id: "PUNCTURE_MOVE",
            follow_up: RAND,
        },
        StateDef::Random {
            id: "RAND",
            branches: &[
                Branch::new(BITE, Repeat::CannotRepeat),
                // `AddBranch(state, 2)`: `CanRepeatXTimes` with `maxTimes` 2.
                Branch::new(PUNCTURE, Repeat::CanRepeatXTimes(2)),
            ],
        },
    ],
    initial: GOOP,
};

/// `HunterKiller`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HunterKiller;

#[sts2_macros::hooks]
impl Hooks for HunterKiller {
    type Me = CreatureId;
}

impl MonsterModel for HunterKiller {
    fn name(&self) -> &'static str {
        "Hunter Killer"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 126, 121)
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
            GOOP => {
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, PowerKind::Tender, Decimal::ONE, Some(me))
            }
            BITE => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 19, 17)),
            PUNCTURE => attack_players_hits(
                eng,
                me,
                if_ascension(ascension, DEADLY_ENEMIES, 8, 7),
                PUNCTURE_REPEAT,
            ),
            _ => unreachable!("not a HunterKiller move: {state}"),
        }
    }
}
