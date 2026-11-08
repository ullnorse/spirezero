//! `SkulkingColony` (`Models.Monsters/SkulkingColony.cs`): Zoom -> Zoom (a second state with the
//! same move) -> Inertia (attack and Strength) -> Piercing Stabs (2 hits) -> Zoom. It enters with
//! 20 `HardenedShell`.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const ZOOM: StateIdx = 0;
const ZOOM_2: StateIdx = 1;
const INERTIA: StateIdx = 2;
const PIERCING_STABS: StateIdx = 3;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "ZOOM_MOVE",
            follow_up: ZOOM_2,
        },
        StateDef::Move {
            id: "ZOOM_MOVE_2",
            follow_up: INERTIA,
        },
        StateDef::Move {
            id: "INERTIA_MOVE",
            follow_up: PIERCING_STABS,
        },
        StateDef::Move {
            id: "PIERCING_STABS_MOVE",
            follow_up: ZOOM,
        },
    ],
    initial: ZOOM,
};

/// `SkulkingColony`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SkulkingColony;

#[sts2_macros::hooks]
impl Hooks for SkulkingColony {
    type Me = CreatureId;
}

impl MonsterModel for SkulkingColony {
    fn name(&self) -> &'static str {
        "Skulking Colony"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 80, 75)
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
            ZOOM | ZOOM_2 => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 16, 14))
            }
            INERTIA => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 11, 9))?;
                let strength = if_ascension(ascension, DEADLY_ENEMIES, 4, 2);
                eng.apply_power(me, PowerKind::Strength, Decimal::from(strength), Some(me))
            }
            // `PiercingStabsRepeat` is 2.
            PIERCING_STABS => {
                attack_players_hits(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 8, 7), 2)
            }
            _ => unreachable!("not a SkulkingColony move: {state}"),
        }
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.apply_power(me, PowerKind::HardenedShell, Decimal::from(20), Some(me))
    }
}
