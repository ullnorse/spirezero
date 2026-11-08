//! `TurretOperator` (`Models.Monsters/TurretOperator.cs`): Unload (5 hits) twice, then Reload
//! (+1 Strength), repeating. `LivingShield`'s `Rampart` gives it Block every turn.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players_hits, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const UNLOAD: StateIdx = 0;
const UNLOAD_2: StateIdx = 1;
const RELOAD: StateIdx = 2;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "UNLOAD_MOVE",
            follow_up: UNLOAD_2,
        },
        StateDef::Move {
            id: "UNLOAD_MOVE_2",
            follow_up: RELOAD,
        },
        StateDef::Move {
            id: "RELOAD_MOVE",
            follow_up: UNLOAD,
        },
    ],
    initial: UNLOAD,
};

/// `TurretOperator`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TurretOperator;

#[sts2_macros::hooks]
impl Hooks for TurretOperator {
    type Me = CreatureId;
}

impl MonsterModel for TurretOperator {
    fn name(&self) -> &'static str {
        "Turret Operator"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 51, 41)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        match state {
            UNLOAD | UNLOAD_2 => {
                let damage = if_ascension(eng.s.ascension, DEADLY_ENEMIES, 4, 3);
                attack_players_hits(eng, me, damage, 5)
            }
            RELOAD => eng.apply_power(me, PowerKind::Strength, Decimal::ONE, Some(me)),
            _ => unreachable!("not a TurretOperator move: {state}"),
        }
    }
}
