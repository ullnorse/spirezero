//! `TerrorEel` (`Models.Monsters/TerrorEel.cs`): Crash <-> Thrash (3 hits, 6 Vigor). It enters with
//! `Shriek`, which stuns it into Terror (99 Vulnerable) once a hit leaves it at or below that many
//! HP. The stun move has no body and is `STUN_MOVE` only in the table.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const CRASH: StateIdx = 0;
const THRASH: StateIdx = 1;
const STUN: StateIdx = 2;
const TERROR: StateIdx = 3;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "CRASH_MOVE",
            follow_up: THRASH,
        },
        StateDef::Move {
            id: "THRASH_MOVE",
            follow_up: CRASH,
        },
        StateDef::Move {
            id: "STUN_MOVE",
            follow_up: TERROR,
        },
        StateDef::Move {
            id: "TERROR_MOVE",
            follow_up: CRASH,
        },
    ],
    initial: CRASH,
};

/// `TerrorEel`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TerrorEel;

impl TerrorEel {
    /// `TerrorState`: the state `Shriek` stuns the eel into.
    pub(crate) const TERROR: StateIdx = TERROR;
}

#[sts2_macros::hooks]
impl Hooks for TerrorEel {
    type Me = CreatureId;
}

impl MonsterModel for TerrorEel {
    fn name(&self) -> &'static str {
        "Terror Eel"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 150, 140)
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
            CRASH => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 18, 16)),
            // `ThrashRepeat` is 3.
            THRASH => {
                attack_players_hits(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 4, 3), 3)?;
                eng.apply_power(me, PowerKind::Vigor, Decimal::from(6), Some(me))
            }
            STUN => Ok(()),
            TERROR => {
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, PowerKind::Vulnerable, Decimal::from(99), Some(me))
            }
            _ => unreachable!("not a TerrorEel move: {state}"),
        }
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        let amount = if_ascension(eng.s.ascension, TOUGH_ENEMIES, 75, 70);
        eng.apply_power(me, PowerKind::Shriek, Decimal::from(amount), Some(me))
    }
}
