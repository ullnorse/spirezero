//! `SlitheringStrangler` (`Models.Monsters/SlitheringStrangler.cs`): opens with Constrict (3
//! Constrict), then a random branch over Thwack (attack, 5 Block) and Lash (attack), both of
//! which can repeat forever and both of which go back to Constrict.

use super::machine::{Branch, MoveTable, Repeat, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, ValueProps};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const RAND: StateIdx = 0;
const THWACK: StateIdx = 1;
const CONSTRICT: StateIdx = 2;
const LASH: StateIdx = 3;

/// The Block Thwack gains.
const THWACK_BLOCK: i32 = 5;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Random {
            id: "rand",
            branches: &[
                Branch::new(THWACK, Repeat::CanRepeatForever),
                Branch::new(LASH, Repeat::CanRepeatForever),
            ],
        },
        StateDef::Move {
            id: "THWACK",
            follow_up: CONSTRICT,
        },
        StateDef::Move {
            id: "CONSTRICT",
            follow_up: RAND,
        },
        StateDef::Move {
            id: "LASH",
            follow_up: CONSTRICT,
        },
    ],
    initial: CONSTRICT,
};

/// `SlitheringStrangler`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SlitheringStrangler;

#[sts2_macros::hooks]
impl Hooks for SlitheringStrangler {
    type Me = CreatureId;
}

impl MonsterModel for SlitheringStrangler {
    fn name(&self) -> &'static str {
        "Slithering Strangler"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 54, 53)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 56, 55)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            CONSTRICT => {
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, PowerKind::Constrict, Decimal::from(3), Some(me))
            }
            THWACK => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 8, 7))?;
                eng.gain_block(me, Decimal::from(THWACK_BLOCK), ValueProps::MOVE, None)?;
                Ok(())
            }
            LASH => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 13, 12)),
            _ => unreachable!("not a Slithering Strangler move: {state}"),
        }
    }
}
