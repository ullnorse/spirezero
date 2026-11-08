//! `TwigSlimeS` (`Models.Monsters/TwigSlimeS.cs`): Tackle every turn.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};

const TACKLE: StateIdx = 0;

static MOVES: MoveTable = MoveTable {
    states: &[StateDef::Move {
        id: "TACKLE_MOVE",
        follow_up: TACKLE,
    }],
    initial: TACKLE,
};

/// `TwigSlimeS`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TwigSlimeS;

#[sts2_macros::hooks]
impl Hooks for TwigSlimeS {
    type Me = CreatureId;
}

impl MonsterModel for TwigSlimeS {
    fn name(&self) -> &'static str {
        "Twig Slime (S)"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 8, 7)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 12, 11)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, _state: StateIdx) -> Flow {
        let damage = if_ascension(eng.s.ascension, DEADLY_ENEMIES, 5, 4);
        attack_players(eng, me, damage)
    }
}
