//! `BygoneEffigy` (`Models.Monsters/BygoneEffigy.cs`): gains 1 Slow when added to the room.
//! Sleeps one turn, then wakes (+10 Strength) and slashes every turn. `SLEEP_MOVE_2` has no
//! incoming transition, so it is left out.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// Strength gained by the wake move.
const WAKE_STRENGTH: i32 = 10;

const SLEEP: StateIdx = 0;
const WAKE: StateIdx = 1;
const SLASHES: StateIdx = 2;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "SLEEP_MOVE",
            follow_up: WAKE,
        },
        StateDef::Move {
            id: "WAKE_MOVE",
            follow_up: SLASHES,
        },
        StateDef::Move {
            id: "SLASHES_MOVE",
            follow_up: SLASHES,
        },
    ],
    initial: SLEEP,
};

/// `BygoneEffigy`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BygoneEffigy;

#[sts2_macros::hooks]
impl Hooks for BygoneEffigy {
    type Me = CreatureId;
}

impl MonsterModel for BygoneEffigy {
    fn name(&self) -> &'static str {
        "Bygone Effigy"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 132, 127)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        match state {
            SLEEP => Ok(()),
            WAKE => eng.apply_power(
                me,
                PowerKind::Strength,
                Decimal::from(WAKE_STRENGTH),
                Some(me),
            ),
            SLASHES => {
                let damage = if_ascension(eng.s.ascension, DEADLY_ENEMIES, 15, 13);
                attack_players(eng, me, damage)
            }
            _ => unreachable!("not a BygoneEffigy move: {state}"),
        }
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.apply_power(me, PowerKind::Slow, Decimal::ONE, Some(me))
    }
}
