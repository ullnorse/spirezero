//! `SnappingJaxfruit` (`Models.Monsters/SnappingJaxfruit.cs`): repeats Energy Orb forever
//! (attack, then 2 Strength). `AfterAddedToRoom`/`BeforeRemovedFromRoom` only play a sound loop.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const ENERGY_ORB: StateIdx = 0;

static MOVES: MoveTable = MoveTable {
    states: &[StateDef::Move {
        id: "ENERGY_ORB_MOVE",
        follow_up: ENERGY_ORB,
    }],
    initial: ENERGY_ORB,
};

/// `SnappingJaxfruit`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SnappingJaxfruit;

#[sts2_macros::hooks]
impl Hooks for SnappingJaxfruit {
    type Me = CreatureId;
}

impl MonsterModel for SnappingJaxfruit {
    fn name(&self) -> &'static str {
        "Snapping Jaxfruit"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 34, 31)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 36, 33)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        match state {
            ENERGY_ORB => {
                let damage = if_ascension(eng.s.ascension, DEADLY_ENEMIES, 4, 3);
                attack_players(eng, me, damage)?;
                eng.apply_power(me, PowerKind::Strength, Decimal::from(2), Some(me))
            }
            _ => unreachable!("not a Snapping Jaxfruit move: {state}"),
        }
    }
}
