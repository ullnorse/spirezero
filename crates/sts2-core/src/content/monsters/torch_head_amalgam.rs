//! `TorchHeadAmalgam` (`Models.Monsters/TorchHeadAmalgam.cs`): the `Queen`'s Minion. Strong Tackle
//! -> Tackle -> Soul Beam (three hits) -> Tackle (weak, `TACKLE_3_MOVE`) -> Tackle (weak,
//! `TACKLE_4_MOVE`) -> Soul Beam... `OnDieToDoom` and the beam's aim are visual.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES};
use super::{attack_players, attack_players_hits, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const STRONG_TACKLE: StateIdx = 0;
const TACKLE_2: StateIdx = 1;
const BEAM: StateIdx = 2;
const TACKLE_3: StateIdx = 3;
const TACKLE_4: StateIdx = 4;

/// `_soulBeamRepeat`.
const SOUL_BEAM_REPEAT: u32 = 3;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "STRONG_TACKLE_MOVE",
            follow_up: TACKLE_2,
        },
        StateDef::Move {
            id: "TACKLE_2_MOVE",
            follow_up: BEAM,
        },
        StateDef::Move {
            id: "BEAM_MOVE",
            follow_up: TACKLE_3,
        },
        StateDef::Move {
            id: "TACKLE_3_MOVE",
            follow_up: TACKLE_4,
        },
        StateDef::Move {
            id: "TACKLE_4_MOVE",
            follow_up: BEAM,
        },
    ],
    initial: STRONG_TACKLE,
};

/// `TorchHeadAmalgam`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TorchHeadAmalgam;

#[sts2_macros::hooks]
impl Hooks for TorchHeadAmalgam {
    type Me = CreatureId;
}

impl MonsterModel for TorchHeadAmalgam {
    fn name(&self) -> &'static str {
        "Torch Head Amalgam"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 211, 199)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.apply_power(me, PowerKind::Minion, Decimal::ONE, Some(me))
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            STRONG_TACKLE => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 32, 26))
            }
            TACKLE_2 => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 22, 18)),
            BEAM => {
                // `GetValueIfAscension(DeadlyEnemies, 8, 8)`.
                let damage = if_ascension(ascension, DEADLY_ENEMIES, 8, 8);
                attack_players_hits(eng, me, damage, SOUL_BEAM_REPEAT)
            }
            TACKLE_3 | TACKLE_4 => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 16, 14))
            }
            _ => unreachable!("not a TorchHeadAmalgam move: {state}"),
        }
    }
}
