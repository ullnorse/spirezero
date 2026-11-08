//! `BowlbugNectar` (`Models.Monsters/BowlbugNectar.cs`): Thrash (3 damage), Buff (+15 Strength,
//! +16 at ascension 9), then Thrash forever. The list order of the C# machine is irrelevant.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const THRASH: StateIdx = 0;
const BUFF: StateIdx = 1;
const THRASH2: StateIdx = 2;

/// `ThrashDamage`.
const THRASH_DAMAGE: i32 = 3;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "THRASH_MOVE",
            follow_up: BUFF,
        },
        StateDef::Move {
            id: "BUFF_MOVE",
            follow_up: THRASH2,
        },
        StateDef::Move {
            id: "THRASH2_MOVE",
            follow_up: THRASH2,
        },
    ],
    initial: THRASH,
};

/// `BowlbugNectar`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BowlbugNectar;

#[sts2_macros::hooks]
impl Hooks for BowlbugNectar {
    type Me = CreatureId;
}

impl MonsterModel for BowlbugNectar {
    fn name(&self) -> &'static str {
        "Bowlbug Nectar"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 36, 35)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 39, 38)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            THRASH | THRASH2 => attack_players(eng, me, THRASH_DAMAGE),
            BUFF => {
                let strength = if_ascension(ascension, DEADLY_ENEMIES, 16, 15);
                eng.apply_power(me, PowerKind::Strength, Decimal::from(strength), Some(me))
            }
            _ => unreachable!("not a BowlbugNectar move: {state}"),
        }
    }
}
