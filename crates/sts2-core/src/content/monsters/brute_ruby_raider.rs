//! `BruteRubyRaider` (`Models.Monsters/BruteRubyRaider.cs`): Beat, then Roar (3 Strength),
//! alternating.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const BEAT: StateIdx = 0;
const ROAR: StateIdx = 1;

/// `_roarStrength`.
const ROAR_STRENGTH: i32 = 3;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "BEAT_MOVE",
            follow_up: ROAR,
        },
        StateDef::Move {
            id: "ROAR_MOVE",
            follow_up: BEAT,
        },
    ],
    initial: BEAT,
};

/// `BruteRubyRaider`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BruteRubyRaider;

#[sts2_macros::hooks]
impl Hooks for BruteRubyRaider {
    type Me = CreatureId;
}

impl MonsterModel for BruteRubyRaider {
    fn name(&self) -> &'static str {
        "Brute Raider"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 31, 30)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 34, 33)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            BEAT => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 8, 7)),
            ROAR => eng.apply_power(
                me,
                PowerKind::Strength,
                Decimal::from(ROAR_STRENGTH),
                Some(me),
            ),
            _ => unreachable!("not a BruteRubyRaider move: {state}"),
        }
    }
}
