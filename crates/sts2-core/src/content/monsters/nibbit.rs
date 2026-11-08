//! `Nibbit` (`Models.Monsters/Nibbit.cs`): Butt -> Slice -> Hiss -> Butt, after an opener.
//!
//! The machine starts at a conditional branch built from `IsAlone` at `SetUpForCombat`: alone,
//! it opens with Butt; otherwise Hiss when not in front and Slice when in front.

use super::machine::{MoveTable, StateDef, StateIdx, when};
use super::{
    DEADLY_ENEMIES, MonsterKind, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow, ValueProps};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const INIT: StateIdx = 0;
const BUTT: StateIdx = 1;
const SLICE: StateIdx = 2;
const HISS: StateIdx = 3;

const BUTT_MOVE: StateDef = StateDef::Move {
    id: "BUTT_MOVE",
    follow_up: SLICE,
};
const SLICE_MOVE: StateDef = StateDef::Move {
    id: "SLICE_MOVE",
    follow_up: HISS,
};
const HISS_MOVE: StateDef = StateDef::Move {
    id: "HISS_MOVE",
    follow_up: BUTT,
};

static MOVES_ALONE: MoveTable = MoveTable {
    states: &[
        StateDef::Conditional {
            id: "INIT_MOVE",
            branches: &[when(BUTT, is_alone)],
        },
        BUTT_MOVE,
        SLICE_MOVE,
        HISS_MOVE,
    ],
    initial: INIT,
};

static MOVES_GROUPED: MoveTable = MoveTable {
    states: &[
        StateDef::Conditional {
            id: "INIT_MOVE",
            branches: &[when(HISS, is_not_front), when(SLICE, is_front)],
        },
        BUTT_MOVE,
        SLICE_MOVE,
        HISS_MOVE,
    ],
    initial: INIT,
};

/// Per-combat fields, set by the encounter before `SetUpForCombat`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Nibbit {
    /// `IsFront`.
    pub is_front: bool,
    /// `IsAlone`.
    pub is_alone: bool,
}

fn fields(s: &CombatState, id: CreatureId) -> Nibbit {
    match s.creature(id).monster.as_ref().map(|m| m.kind) {
        Some(MonsterKind::Nibbit(n)) => n,
        _ => unreachable!("the owner is a Nibbit"),
    }
}

fn is_alone(s: &CombatState, id: CreatureId) -> bool {
    fields(s, id).is_alone
}

fn is_front(s: &CombatState, id: CreatureId) -> bool {
    fields(s, id).is_front
}

fn is_not_front(s: &CombatState, id: CreatureId) -> bool {
    !fields(s, id).is_front
}

#[sts2_macros::hooks]
impl Hooks for Nibbit {
    type Me = CreatureId;
}

impl MonsterModel for Nibbit {
    fn name(&self) -> &'static str {
        "Nibbit"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 44, 42)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 48, 46)
    }

    fn moves(&self) -> &'static MoveTable {
        if self.is_alone {
            &MOVES_ALONE
        } else {
            &MOVES_GROUPED
        }
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            BUTT => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 13, 12)),
            SLICE => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 7, 6))?;
                let block = if_ascension(ascension, TOUGH_ENEMIES, 6, 5);
                eng.gain_block(me, Decimal::from(block), ValueProps::MOVE, None)?;
                Ok(())
            }
            HISS => {
                let strength = if_ascension(ascension, DEADLY_ENEMIES, 3, 2);
                eng.apply_power(me, PowerKind::Strength, Decimal::from(strength), Some(me))
            }
            _ => unreachable!("not a Nibbit move: {state}"),
        }
    }
}
