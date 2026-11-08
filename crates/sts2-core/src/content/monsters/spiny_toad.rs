//! `SpinyToad` (`Models.Monsters/SpinyToad.cs`): Protruding Spikes (5 Thorns), Spike Explosion
//! (attack, then loses the 5 Thorns) and Tongue Lash (attack) in a cycle. `IsSpiny` only picks
//! animations.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const SPIKES: StateIdx = 0;
const EXPLOSION: StateIdx = 1;
const LASH: StateIdx = 2;

/// The Thorns the spikes grant and the explosion takes back.
const THORNS: i32 = 5;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "PROTRUDING_SPIKES_MOVE",
            follow_up: EXPLOSION,
        },
        StateDef::Move {
            id: "SPIKE_EXPLOSION_MOVE",
            follow_up: LASH,
        },
        StateDef::Move {
            id: "TONGUE_LASH_MOVE",
            follow_up: SPIKES,
        },
    ],
    initial: SPIKES,
};

/// `SpinyToad`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SpinyToad;

#[sts2_macros::hooks]
impl Hooks for SpinyToad {
    type Me = CreatureId;
}

impl MonsterModel for SpinyToad {
    fn name(&self) -> &'static str {
        "Spiny Toad"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 121, 116)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 124, 119)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            SPIKES => eng.apply_power(me, PowerKind::Thorns, Decimal::from(THORNS), Some(me)),
            EXPLOSION => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 25, 23))?;
                eng.apply_power(me, PowerKind::Thorns, Decimal::from(-THORNS), Some(me))
            }
            LASH => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 19, 17)),
            _ => unreachable!("not a SpinyToad move: {state}"),
        }
    }
}
