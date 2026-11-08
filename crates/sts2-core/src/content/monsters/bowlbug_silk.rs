//! `BowlbugSilk` (`Models.Monsters/BowlbugSilk.cs`): opens with Toxic Spit (1 Weak), then Thrash
//! (2 hits) and Toxic Spit alternate.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players_hits, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const THRASH: StateIdx = 0;
const TOXIC_SPIT: StateIdx = 1;

/// `_thrashRepeat`.
const THRASH_REPEAT: u32 = 2;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "THRASH_MOVE",
            follow_up: TOXIC_SPIT,
        },
        StateDef::Move {
            id: "TOXIC_SPIT_MOVE",
            follow_up: THRASH,
        },
    ],
    initial: TOXIC_SPIT,
};

/// `BowlbugSilk`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BowlbugSilk;

#[sts2_macros::hooks]
impl Hooks for BowlbugSilk {
    type Me = CreatureId;
}

impl MonsterModel for BowlbugSilk {
    fn name(&self) -> &'static str {
        "Bowlbug Silk"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 41, 40)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 44, 43)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            THRASH => attack_players_hits(
                eng,
                me,
                if_ascension(ascension, DEADLY_ENEMIES, 5, 4),
                THRASH_REPEAT,
            ),
            TOXIC_SPIT => {
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, PowerKind::Weak, Decimal::ONE, Some(me))
            }
            _ => unreachable!("not a BowlbugSilk move: {state}"),
        }
    }
}
