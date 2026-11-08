//! `Seapunk` (`Models.Monsters/Seapunk.cs`): Sea Kick -> Spinning Kick (4 hits) -> Bubble Burp
//! (block and Strength) -> Sea Kick.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, ValueProps};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const SEA_KICK: StateIdx = 0;
const SPINNING_KICK: StateIdx = 1;
const BUBBLE_BURP: StateIdx = 2;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "SEA_KICK_MOVE",
            follow_up: SPINNING_KICK,
        },
        StateDef::Move {
            id: "SPINNING_KICK_MOVE",
            follow_up: BUBBLE_BURP,
        },
        StateDef::Move {
            id: "BUBBLE_BURP_MOVE",
            follow_up: SEA_KICK,
        },
    ],
    initial: SEA_KICK,
};

/// `Seapunk`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Seapunk;

#[sts2_macros::hooks]
impl Hooks for Seapunk {
    type Me = CreatureId;
}

impl MonsterModel for Seapunk {
    fn name(&self) -> &'static str {
        "Seapunk"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 47, 44)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 49, 46)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            SEA_KICK => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 13, 11)),
            // `SpinningKickDamage` 2, `SpinningKickRepeat` 4.
            SPINNING_KICK => attack_players_hits(eng, me, 2, 4),
            BUBBLE_BURP => {
                let block = if_ascension(ascension, TOUGH_ENEMIES, 8, 7);
                eng.gain_block(me, Decimal::from(block), ValueProps::MOVE, None)?;
                let strength = if_ascension(ascension, DEADLY_ENEMIES, 2, 1);
                eng.apply_power(me, PowerKind::Strength, Decimal::from(strength), Some(me))
            }
            _ => unreachable!("not a Seapunk move: {state}"),
        }
    }
}
