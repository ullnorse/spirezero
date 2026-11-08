//! `KinPriest` (`Models.Monsters/KinPriest.cs`): Orb of Frailty (attack, 1 Frail), Orb of Weakness
//! (attack, 1 Weak), Beam (3 hits) and Ritual (+Strength), in a cycle. Its `AfterDeath` only
//! plays music and a speech line; `SpeechUsed` is never read.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `_beamRepeat`.
const BEAM_REPEAT: u32 = 3;

const ORB_OF_FRAILTY: StateIdx = 0;
const ORB_OF_WEAKNESS: StateIdx = 1;
const BEAM: StateIdx = 2;
const RITUAL: StateIdx = 3;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "ORB_OF_FRAILTY_MOVE",
            follow_up: ORB_OF_WEAKNESS,
        },
        StateDef::Move {
            id: "ORB_OF_WEAKNESS_MOVE",
            follow_up: BEAM,
        },
        StateDef::Move {
            id: "BEAM_MOVE",
            follow_up: RITUAL,
        },
        StateDef::Move {
            id: "RITUAL_MOVE",
            follow_up: ORB_OF_FRAILTY,
        },
    ],
    initial: ORB_OF_FRAILTY,
};

/// `KinPriest`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct KinPriest;

#[sts2_macros::hooks]
impl Hooks for KinPriest {
    type Me = CreatureId;
}

impl MonsterModel for KinPriest {
    fn name(&self) -> &'static str {
        "Kin Priest"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 199, 190)
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
            ORB_OF_FRAILTY | ORB_OF_WEAKNESS => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 9, 8))?;
                let debuff = if state == ORB_OF_FRAILTY {
                    PowerKind::Frail
                } else {
                    PowerKind::Weak
                };
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, debuff, Decimal::ONE, Some(me))
            }
            // `BeamDamage` is 3 at every ascension.
            BEAM => attack_players_hits(eng, me, 3, BEAM_REPEAT),
            RITUAL => {
                let strength = if_ascension(ascension, DEADLY_ENEMIES, 3, 2);
                eng.apply_power(me, PowerKind::Strength, Decimal::from(strength), Some(me))
            }
            _ => unreachable!("not a KinPriest move: {state}"),
        }
    }
}
