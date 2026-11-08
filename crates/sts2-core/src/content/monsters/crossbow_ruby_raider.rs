//! `CrossbowRubyRaider` (`Models.Monsters/CrossbowRubyRaider.cs`): Reload (3 block) first, then
//! Fire, alternating. `IsCrossbowReloaded` only drives the animations, so it is not kept.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, ValueProps};
use crate::decimal::Decimal;

const RELOAD: StateIdx = 0;
const FIRE: StateIdx = 1;

/// Reload's block.
const RELOAD_BLOCK: i32 = 3;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "RELOAD_MOVE",
            follow_up: FIRE,
        },
        StateDef::Move {
            id: "FIRE_MOVE",
            follow_up: RELOAD,
        },
    ],
    initial: RELOAD,
};

/// `CrossbowRubyRaider`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CrossbowRubyRaider;

#[sts2_macros::hooks]
impl Hooks for CrossbowRubyRaider {
    type Me = CreatureId;
}

impl MonsterModel for CrossbowRubyRaider {
    fn name(&self) -> &'static str {
        "Crossbow Raider"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 19, 18)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 22, 21)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            FIRE => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 16, 14)),
            RELOAD => {
                eng.gain_block(me, Decimal::from(RELOAD_BLOCK), ValueProps::MOVE, None)?;
                Ok(())
            }
            _ => unreachable!("not a CrossbowRubyRaider move: {state}"),
        }
    }
}
