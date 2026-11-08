//! `Myte` (`Models.Monsters/Myte.cs`): Toxic (2 `Toxic` into the hand), Bite and Suck (attack,
//! +Strength) in a cycle. The `first` Myte opens with Toxic, the `second` with Suck.

use super::machine::{MoveTable, StateDef, StateIdx, when};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow, PileType};
use crate::content::cards::CardKind;
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const INIT: StateIdx = 0;
const TOXIC: StateIdx = 1;
const BITE: StateIdx = 2;
const SUCK: StateIdx = 3;

/// `_toxicCount`.
const TOXIC_COUNT: u32 = 2;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Conditional {
            id: "INIT_MOVE",
            branches: &[when(TOXIC, in_first), when(SUCK, in_second)],
        },
        StateDef::Move {
            id: "TOXIC_MOVE",
            follow_up: BITE,
        },
        StateDef::Move {
            id: "BITE_MOVE",
            follow_up: SUCK,
        },
        StateDef::Move {
            id: "SUCK_MOVE",
            follow_up: TOXIC,
        },
    ],
    initial: INIT,
};

fn in_first(s: &CombatState, id: CreatureId) -> bool {
    s.creature(id).slot == Some("first")
}

fn in_second(s: &CombatState, id: CreatureId) -> bool {
    s.creature(id).slot == Some("second")
}

/// `Myte`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Myte;

#[sts2_macros::hooks]
impl Hooks for Myte {
    type Me = CreatureId;
}

impl MonsterModel for Myte {
    fn name(&self) -> &'static str {
        "Myte"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 64, 61)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 69, 67)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            TOXIC => {
                for target in eng.s.player_creatures() {
                    eng.add_to_combat_and_preview(
                        target,
                        CardKind::Toxic,
                        PileType::Hand,
                        TOXIC_COUNT,
                        None,
                    )?;
                }
                Ok(())
            }
            BITE => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 15, 13)),
            SUCK => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 6, 4))?;
                let strength = if_ascension(ascension, DEADLY_ENEMIES, 3, 2);
                eng.apply_power(me, PowerKind::Strength, Decimal::from(strength), Some(me))
            }
            _ => unreachable!("not a Myte move: {state}"),
        }
    }
}
