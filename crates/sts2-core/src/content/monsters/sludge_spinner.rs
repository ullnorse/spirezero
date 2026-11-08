//! `SludgeSpinner` (`Models.Monsters/SludgeSpinner.cs`): Oil Spray (1 Weak) first, then a random
//! branch over Oil Spray, Slam and Rage (3 Strength), none twice in a row.

use super::machine::{Branch, MoveTable, Repeat, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const RAND: StateIdx = 0;
const OIL_SPRAY: StateIdx = 1;
const SLAM: StateIdx = 2;
const RAGE: StateIdx = 3;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Random {
            id: "RAND",
            branches: &[
                Branch::new(OIL_SPRAY, Repeat::CannotRepeat),
                Branch::new(SLAM, Repeat::CannotRepeat),
                Branch::new(RAGE, Repeat::CannotRepeat),
            ],
        },
        StateDef::Move {
            id: "OIL_SPRAY_MOVE",
            follow_up: RAND,
        },
        StateDef::Move {
            id: "SLAM_MOVE",
            follow_up: RAND,
        },
        StateDef::Move {
            id: "RAGE_MOVE",
            follow_up: RAND,
        },
    ],
    initial: OIL_SPRAY,
};

/// `SludgeSpinner`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SludgeSpinner;

#[sts2_macros::hooks]
impl Hooks for SludgeSpinner {
    type Me = CreatureId;
}

impl MonsterModel for SludgeSpinner {
    fn name(&self) -> &'static str {
        "Sludge Spinner"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 41, 37)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 42, 39)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            OIL_SPRAY => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 9, 8))?;
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, PowerKind::Weak, Decimal::ONE, Some(me))
            }
            SLAM => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 12, 11)),
            RAGE => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 7, 6))?;
                eng.apply_power(me, PowerKind::Strength, Decimal::from(3), Some(me))
            }
            _ => unreachable!("not a SludgeSpinner move: {state}"),
        }
    }
}
