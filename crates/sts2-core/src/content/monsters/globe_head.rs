//! `GlobeHead` (`Models.Monsters/GlobeHead.cs`): starts with Galvanic. Shocking Slap (damage,
//! 2 Frail) -> Thunder Strike (3 hits) -> Galvanic Burst (damage, 2 Strength) -> Shocking Slap.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const SHOCKING_SLAP: StateIdx = 0;
const THUNDER_STRIKE: StateIdx = 1;
const GALVANIC_BURST: StateIdx = 2;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "SHOCKING_SLAP",
            follow_up: THUNDER_STRIKE,
        },
        StateDef::Move {
            id: "THUNDER_STRIKE",
            follow_up: GALVANIC_BURST,
        },
        StateDef::Move {
            id: "GALVANIC_BURST",
            follow_up: SHOCKING_SLAP,
        },
    ],
    initial: SHOCKING_SLAP,
};

/// `GlobeHead`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct GlobeHead;

#[sts2_macros::hooks]
impl Hooks for GlobeHead {
    type Me = CreatureId;
}

impl MonsterModel for GlobeHead {
    fn name(&self) -> &'static str {
        "Globe Head"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 158, 148)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        let amount = if_ascension(eng.s.ascension, DEADLY_ENEMIES, 8, 6);
        eng.apply_power(me, PowerKind::Galvanic, Decimal::from(amount), Some(me))
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            THUNDER_STRIKE => {
                attack_players_hits(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 7, 6), 3)
            }
            SHOCKING_SLAP => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 14, 13))?;
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, PowerKind::Frail, Decimal::from(2), Some(me))
            }
            GALVANIC_BURST => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 17, 16))?;
                eng.apply_power(me, PowerKind::Strength, Decimal::from(2), Some(me))
            }
            _ => unreachable!("not a GlobeHead move: {state}"),
        }
    }
}
