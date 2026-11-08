//! `DampCultist` (`Models.Monsters/DampCultist.cs`): Incantation (5 Ritual, 6 from
//! `DeadlyEnemies`), then Dark Strike forever.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const INCANTATION: StateIdx = 0;
const DARK_STRIKE: StateIdx = 1;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "INCANTATION_MOVE",
            follow_up: DARK_STRIKE,
        },
        StateDef::Move {
            id: "DARK_STRIKE_MOVE",
            follow_up: DARK_STRIKE,
        },
    ],
    initial: INCANTATION,
};

/// `DampCultist`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DampCultist;

#[sts2_macros::hooks]
impl Hooks for DampCultist {
    type Me = CreatureId;
}

impl MonsterModel for DampCultist {
    fn name(&self) -> &'static str {
        "Damp Cultist"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 52, 51)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 54, 53)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            INCANTATION => {
                let amount = if_ascension(ascension, DEADLY_ENEMIES, 6, 5);
                eng.apply_power(me, PowerKind::Ritual, Decimal::from(amount), Some(me))
            }
            DARK_STRIKE => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 3, 1)),
            _ => unreachable!("not a DampCultist move: {state}"),
        }
    }
}
