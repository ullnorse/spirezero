//! `AssassinRubyRaider` (`Models.Monsters/AssassinRubyRaider.cs`): Killshot every turn.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};

const KILLSHOT: StateIdx = 0;

static MOVES: MoveTable = MoveTable {
    states: &[StateDef::Move {
        id: "KILLSHOT_MOVE",
        follow_up: KILLSHOT,
    }],
    initial: KILLSHOT,
};

/// `AssassinRubyRaider`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AssassinRubyRaider;

#[sts2_macros::hooks]
impl Hooks for AssassinRubyRaider {
    type Me = CreatureId;
}

impl MonsterModel for AssassinRubyRaider {
    fn name(&self) -> &'static str {
        "Assassin Raider"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 19, 18)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 24, 23)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            KILLSHOT => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 11, 10)),
            _ => unreachable!("not an AssassinRubyRaider move: {state}"),
        }
    }
}
