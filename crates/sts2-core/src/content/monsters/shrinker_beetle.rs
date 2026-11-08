//! `ShrinkerBeetle` (`Models.Monsters/ShrinkerBeetle.cs`): Shrinker (infinite `ShrinkPower` on
//! the players), then Chomp and Stomp alternating. No random branches.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const SHRINKER: StateIdx = 0;
const CHOMP: StateIdx = 1;
const STOMP: StateIdx = 2;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "SHRINKER_MOVE",
            follow_up: CHOMP,
        },
        StateDef::Move {
            id: "CHOMP_MOVE",
            follow_up: STOMP,
        },
        StateDef::Move {
            id: "STOMP_MOVE",
            follow_up: CHOMP,
        },
    ],
    initial: SHRINKER,
};

/// `ShrinkerBeetle`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ShrinkerBeetle;

#[sts2_macros::hooks]
impl Hooks for ShrinkerBeetle {
    type Me = CreatureId;
}

impl MonsterModel for ShrinkerBeetle {
    fn name(&self) -> &'static str {
        "Shrinker Beetle"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 40, 38)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 42, 40)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            SHRINKER => {
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, PowerKind::Shrink, Decimal::from(-1), Some(me))
            }
            CHOMP => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 8, 7)),
            STOMP => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 14, 13)),
            _ => unreachable!("not a ShrinkerBeetle move: {state}"),
        }
    }
}
