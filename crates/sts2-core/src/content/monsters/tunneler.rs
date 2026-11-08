//! `Tunneler` (`Models.Monsters/Tunneler.cs`): Bite, then Burrow (gains `Burrowed` and block),
//! then Below (an attack from underground) forever. `BurrowedPower` stuns it back into Bite when
//! its block is broken. `DIZZY_MOVE`, the move that stun passes, is never entered as a state
//! and does nothing but clear `IsStunned`, which only picks animations; neither is kept.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, ValueProps};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const BITE: StateIdx = 0;
const BURROW: StateIdx = 1;
const BELOW: StateIdx = 2;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "BITE_MOVE",
            follow_up: BURROW,
        },
        StateDef::Move {
            id: "BURROW_MOVE",
            follow_up: BELOW,
        },
        StateDef::Move {
            id: "BELOW_MOVE",
            follow_up: BELOW,
        },
    ],
    initial: BITE,
};

/// `Tunneler`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Tunneler;

impl Tunneler {
    /// The state `BITE_MOVE`, which `BurrowedPower` stuns the Tunneler into.
    pub const BITE: StateIdx = BITE;
}

#[sts2_macros::hooks]
impl Hooks for Tunneler {
    type Me = CreatureId;
}

impl MonsterModel for Tunneler {
    fn name(&self) -> &'static str {
        "Tunneler"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 92, 87)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            BITE => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 15, 13)),
            BURROW => {
                eng.apply_power(me, PowerKind::Burrowed, Decimal::ONE, Some(me))?;
                let block = if_ascension(ascension, TOUGH_ENEMIES, 37, 32);
                eng.gain_block(me, Decimal::from(block), ValueProps::MOVE, None)?;
                Ok(())
            }
            BELOW => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 26, 23)),
            _ => unreachable!("not a Tunneler move: {state}"),
        }
    }
}
