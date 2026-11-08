//! `FossilStalker` (`Models.Monsters/FossilStalker.cs`): opens with Latch, then a random branch over
//! Latch, Tackle and Lash (each at most twice in a row). Enters with 3 `Suck`.

use super::machine::{Branch, MoveTable, Repeat, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const RAND: StateIdx = 0;
const TACKLE: StateIdx = 1;
const LATCH: StateIdx = 2;
const LASH: StateIdx = 3;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Random {
            id: "RAND",
            branches: &[
                // `AddBranch(state, 2)`: `CanRepeatXTimes` with `maxTimes` 2.
                Branch::new(LATCH, Repeat::CanRepeatXTimes(2)),
                Branch::new(TACKLE, Repeat::CanRepeatXTimes(2)),
                Branch::new(LASH, Repeat::CanRepeatXTimes(2)),
            ],
        },
        StateDef::Move {
            id: "TACKLE_MOVE",
            follow_up: RAND,
        },
        StateDef::Move {
            id: "LATCH_MOVE",
            follow_up: RAND,
        },
        StateDef::Move {
            id: "LASH_MOVE",
            follow_up: RAND,
        },
    ],
    initial: LATCH,
};

/// `FossilStalker`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FossilStalker;

#[sts2_macros::hooks]
impl Hooks for FossilStalker {
    type Me = CreatureId;
}

impl MonsterModel for FossilStalker {
    fn name(&self) -> &'static str {
        "Fossil Stalker"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 54, 51)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 56, 53)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            TACKLE => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 11, 9))?;
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, PowerKind::Frail, Decimal::ONE, Some(me))
            }
            LATCH => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 14, 12)),
            // `LashRepeat` is 2.
            LASH => attack_players_hits(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 4, 3), 2),
            _ => unreachable!("not a FossilStalker move: {state}"),
        }
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.apply_power(me, PowerKind::Suck, Decimal::from(3), Some(me))
    }
}
