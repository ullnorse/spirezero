//! `TwigSlimeM` (`Models.Monsters/TwigSlimeM.cs`): Sticky Shot (1 Slimed into the discard pile)
//! first, then a random branch: Pokey Pounce (at most twice in a row) or Sticky Shot (never
//! twice in a row).

use super::machine::{Branch, MoveTable, Repeat, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};

const POKEY_POUNCE: StateIdx = 0;
const STICKY_SHOT: StateIdx = 1;
const RAND: StateIdx = 2;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "POKEY_POUNCE_MOVE",
            follow_up: RAND,
        },
        StateDef::Move {
            id: "STICKY_SHOT_MOVE",
            follow_up: RAND,
        },
        StateDef::Random {
            id: "RAND",
            branches: &[
                // `AddBranch(state, 2)`: `CanRepeatXTimes` with `maxTimes` 2.
                Branch::new(POKEY_POUNCE, Repeat::CanRepeatXTimes(2)),
                Branch::new(STICKY_SHOT, Repeat::CannotRepeat),
            ],
        },
    ],
    initial: STICKY_SHOT,
};

/// `TwigSlimeM`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TwigSlimeM;

#[sts2_macros::hooks]
impl Hooks for TwigSlimeM {
    type Me = CreatureId;
}

impl MonsterModel for TwigSlimeM {
    fn name(&self) -> &'static str {
        "Twig Slime (M)"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 27, 26)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 29, 28)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        match state {
            POKEY_POUNCE => {
                let damage = if_ascension(eng.s.ascension, DEADLY_ENEMIES, 12, 11);
                attack_players(eng, me, damage)
            }
            STICKY_SHOT => super::add_slimed(eng, 1),
            _ => unreachable!("not a TwigSlimeM move: {state}"),
        }
    }
}
