//! `Exoskeleton` (`Models.Monsters/Exoskeleton.cs`): 9 `HardToKill` when added to the room. The
//! opening move depends on the slot: Skitter (`first`), Mandibles (`second`), Enrage (`third`) or
//! the random branch (`fourth`). Skitter goes to the random branch (Skitter or Mandibles, neither
//! twice in a row), Mandibles to Enrage, Enrage to the random branch.

use super::machine::{Branch, MoveTable, Repeat, StateDef, StateIdx, when};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const INIT: StateIdx = 0;
const RAND: StateIdx = 1;
const SKITTER: StateIdx = 2;
const MANDIBLES: StateIdx = 3;
const ENRAGE: StateIdx = 4;

/// `_buffAmount`.
const BUFF_AMOUNT: i32 = 2;
/// `SkitterDamage`.
const SKITTER_DAMAGE: i32 = 1;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Conditional {
            id: "INIT_MOVE",
            branches: &[
                when(SKITTER, in_first),
                when(MANDIBLES, in_second),
                when(ENRAGE, in_third),
                when(RAND, in_fourth),
            ],
        },
        StateDef::Random {
            id: "RAND",
            branches: &[
                Branch::new(SKITTER, Repeat::CannotRepeat),
                Branch::new(MANDIBLES, Repeat::CannotRepeat),
            ],
        },
        StateDef::Move {
            id: "SKITTER_MOVE",
            follow_up: RAND,
        },
        StateDef::Move {
            id: "MANDIBLES_MOVE",
            follow_up: ENRAGE,
        },
        StateDef::Move {
            id: "ENRAGE_MOVE",
            follow_up: RAND,
        },
    ],
    initial: INIT,
};

fn in_slot(s: &CombatState, id: CreatureId, slot: &str) -> bool {
    s.creature(id).slot == Some(slot)
}

fn in_first(s: &CombatState, id: CreatureId) -> bool {
    in_slot(s, id, "first")
}

fn in_second(s: &CombatState, id: CreatureId) -> bool {
    in_slot(s, id, "second")
}

fn in_third(s: &CombatState, id: CreatureId) -> bool {
    in_slot(s, id, "third")
}

fn in_fourth(s: &CombatState, id: CreatureId) -> bool {
    in_slot(s, id, "fourth")
}

/// `Exoskeleton`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Exoskeleton;

#[sts2_macros::hooks]
impl Hooks for Exoskeleton {
    type Me = CreatureId;
}

impl MonsterModel for Exoskeleton {
    fn name(&self) -> &'static str {
        "Exoskeleton"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 26, 24)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 30, 28)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            SKITTER => attack_players_hits(
                eng,
                me,
                SKITTER_DAMAGE,
                u32::try_from(if_ascension(ascension, DEADLY_ENEMIES, 4, 3))
                    .expect("a positive hit count"),
            ),
            MANDIBLES => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 9, 8)),
            ENRAGE => eng.apply_power(
                me,
                PowerKind::Strength,
                Decimal::from(BUFF_AMOUNT),
                Some(me),
            ),
            _ => unreachable!("not an Exoskeleton move: {state}"),
        }
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.apply_power(me, PowerKind::HardToKill, Decimal::from(9), Some(me))
    }
}
