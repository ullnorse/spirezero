//! `AxeRubyRaider` (`Models.Monsters/AxeRubyRaider.cs`): Swing (attack and block), Swing again,
//! then Big Swing, repeating.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, ValueProps};
use crate::decimal::Decimal;

const SWING_1: StateIdx = 0;
const SWING_2: StateIdx = 1;
const BIG_SWING: StateIdx = 2;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "SWING_1",
            follow_up: SWING_2,
        },
        StateDef::Move {
            id: "SWING_2",
            follow_up: BIG_SWING,
        },
        StateDef::Move {
            id: "BIG_SWING",
            follow_up: SWING_1,
        },
    ],
    initial: SWING_1,
};

/// `AxeRubyRaider`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AxeRubyRaider;

#[sts2_macros::hooks]
impl Hooks for AxeRubyRaider {
    type Me = CreatureId;
}

impl MonsterModel for AxeRubyRaider {
    fn name(&self) -> &'static str {
        "Axe Raider"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 21, 20)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 23, 22)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            SWING_1 | SWING_2 => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 6, 5))?;
                let block = if_ascension(ascension, DEADLY_ENEMIES, 6, 5);
                eng.gain_block(me, Decimal::from(block), ValueProps::MOVE, None)?;
                Ok(())
            }
            BIG_SWING => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 13, 12)),
            _ => unreachable!("not an AxeRubyRaider move: {state}"),
        }
    }
}
