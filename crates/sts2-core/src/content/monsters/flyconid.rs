//! `Flyconid` (`Models.Monsters/Flyconid.cs`): opens with a random pick of Frail Spores or
//! Smash, then a random branch over Vulnerable Spores (2 Vulnerable), Frail Spores (attack, then
//! 2 Frail) and Smash, none twice in a row; the spores also have cooldowns.

use super::machine::{Branch, MoveTable, Repeat, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const VULNERABLE_SPORES: StateIdx = 0;
const FRAIL_SPORES: StateIdx = 1;
const SMASH: StateIdx = 2;
const RAND: StateIdx = 3;
const INITIAL: StateIdx = 4;

/// `AddBranch(state, cooldown, repeatType)`: weight 1.
const fn branch(state: StateIdx, cooldown: u32, repeat: Repeat) -> Branch {
    Branch {
        cooldown,
        ..Branch::new(state, repeat)
    }
}

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "VULNERABLE_SPORES_MOVE",
            follow_up: RAND,
        },
        StateDef::Move {
            id: "FRAIL_SPORES_MOVE",
            follow_up: RAND,
        },
        StateDef::Move {
            id: "SMASH_MOVE",
            follow_up: RAND,
        },
        StateDef::Random {
            id: "RAND",
            branches: &[
                branch(VULNERABLE_SPORES, 3, Repeat::CannotRepeat),
                branch(FRAIL_SPORES, 2, Repeat::CannotRepeat),
                Branch::new(SMASH, Repeat::CannotRepeat),
            ],
        },
        StateDef::Random {
            id: "INITIAL",
            branches: &[
                branch(FRAIL_SPORES, 2, Repeat::CannotRepeat),
                Branch::new(SMASH, Repeat::CannotRepeat),
            ],
        },
    ],
    initial: INITIAL,
};

/// `Flyconid`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Flyconid;

#[sts2_macros::hooks]
impl Hooks for Flyconid {
    type Me = CreatureId;
}

impl MonsterModel for Flyconid {
    fn name(&self) -> &'static str {
        "Flyconid"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 51, 47)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 53, 49)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            VULNERABLE_SPORES => {
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, PowerKind::Vulnerable, Decimal::from(2), Some(me))
            }
            FRAIL_SPORES => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 9, 8))?;
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, PowerKind::Frail, Decimal::from(2), Some(me))
            }
            SMASH => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 12, 11)),
            _ => unreachable!("not a Flyconid move: {state}"),
        }
    }
}
