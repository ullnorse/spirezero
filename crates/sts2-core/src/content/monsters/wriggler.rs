//! `Wriggler` (`Models.Monsters/Wriggler.cs`): spawned by `InfestedPower` with `StartStunned`,
//! which opens with the Spawned move (a stun, no effect); otherwise it opens at `INIT_MOVE`. That
//! conditional branch picks Nasty Bite in the `wriggler1` and `wriggler3` slots and Wriggle in
//! `wriggler2` and `wriggler4`; Nasty Bite and Wriggle then alternate.

use super::machine::{MoveTable, StateDef, StateIdx, when};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, add_status_cards, attack_players, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow};
use crate::content::cards::CardKind;
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `_wriggleStrength`.
const WRIGGLE_STRENGTH: i32 = 2;

const SPAWNED: StateIdx = 0;
const NASTY_BITE: StateIdx = 1;
const WRIGGLE: StateIdx = 2;
const INIT: StateIdx = 3;

const STATES: &[StateDef] = &[
    StateDef::Move {
        id: "SPAWNED_MOVE",
        follow_up: INIT,
    },
    StateDef::Move {
        id: "NASTY_BITE_MOVE",
        follow_up: WRIGGLE,
    },
    StateDef::Move {
        id: "WRIGGLE_MOVE",
        follow_up: NASTY_BITE,
    },
    StateDef::Conditional {
        id: "INIT_MOVE",
        branches: &[
            when(NASTY_BITE, in_slot_1),
            when(WRIGGLE, in_slot_2),
            when(NASTY_BITE, in_slot_3),
            when(WRIGGLE, in_slot_4),
        ],
    },
];

static MOVES: MoveTable = MoveTable {
    states: STATES,
    initial: INIT,
};

static MOVES_STUNNED: MoveTable = MoveTable {
    states: STATES,
    initial: SPAWNED,
};

fn in_slot(s: &CombatState, id: CreatureId, slot: &str) -> bool {
    s.creature(id).slot == Some(slot)
}

fn in_slot_1(s: &CombatState, id: CreatureId) -> bool {
    in_slot(s, id, "wriggler1")
}

fn in_slot_2(s: &CombatState, id: CreatureId) -> bool {
    in_slot(s, id, "wriggler2")
}

fn in_slot_3(s: &CombatState, id: CreatureId) -> bool {
    in_slot(s, id, "wriggler3")
}

fn in_slot_4(s: &CombatState, id: CreatureId) -> bool {
    in_slot(s, id, "wriggler4")
}

/// Per-combat fields, set before `SetUpForCombat`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Wriggler {
    /// `StartStunned`: opens with the Spawned move.
    pub start_stunned: bool,
}

#[sts2_macros::hooks]
impl Hooks for Wriggler {
    type Me = CreatureId;
}

impl MonsterModel for Wriggler {
    fn name(&self) -> &'static str {
        "Wriggler"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 18, 17)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 22, 21)
    }

    fn moves(&self) -> &'static MoveTable {
        if self.start_stunned {
            &MOVES_STUNNED
        } else {
            &MOVES
        }
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        match state {
            SPAWNED => Ok(()),
            NASTY_BITE => {
                let damage = if_ascension(eng.s.ascension, DEADLY_ENEMIES, 7, 6);
                attack_players(eng, me, damage)
            }
            WRIGGLE => {
                add_status_cards(eng, CardKind::Infection, 1)?;
                eng.apply_power(
                    me,
                    PowerKind::Strength,
                    Decimal::from(WRIGGLE_STRENGTH),
                    Some(me),
                )
            }
            _ => unreachable!("not a Wriggler move: {state}"),
        }
    }
}
