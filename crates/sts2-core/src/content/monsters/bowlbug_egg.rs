//! `BowlbugEgg` (`Models.Monsters/BowlbugEgg.cs`): one move, Bite (attack and block), forever.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, ValueProps};
use crate::decimal::Decimal;

const BITE: StateIdx = 0;

static MOVES: MoveTable = MoveTable {
    states: &[StateDef::Move {
        id: "BITE_MOVE",
        follow_up: BITE,
    }],
    initial: BITE,
};

/// `BowlbugEgg`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BowlbugEgg;

#[sts2_macros::hooks]
impl Hooks for BowlbugEgg {
    type Me = CreatureId;
}

impl MonsterModel for BowlbugEgg {
    fn name(&self) -> &'static str {
        "Bowlbug Egg"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 23, 21)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 24, 22)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            BITE => {
                // `BiteDamage` and `ProtectBlock` have the same values.
                let amount = if_ascension(ascension, DEADLY_ENEMIES, 8, 7);
                attack_players(eng, me, amount)?;
                eng.gain_block(me, Decimal::from(amount), ValueProps::MOVE, None)?;
                Ok(())
            }
            _ => unreachable!("not a BowlbugEgg move: {state}"),
        }
    }
}
