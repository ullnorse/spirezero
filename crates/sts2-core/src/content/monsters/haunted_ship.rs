//! `HauntedShip` (`Models.Monsters/HauntedShip.cs`): Haunt (3 Weak and 5 Dazed into the discard
//! pile) first, then Swipe and Stomp alternating.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, add_status_cards, attack_players,
    attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::cards::CardKind;
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const SWIPE: StateIdx = 0;
const STOMP: StateIdx = 1;
const HAUNT: StateIdx = 2;

/// `HauntDazed`.
const HAUNT_DAZED: u32 = 5;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "SWIPE_MOVE",
            follow_up: STOMP,
        },
        StateDef::Move {
            id: "STOMP_MOVE",
            follow_up: SWIPE,
        },
        StateDef::Move {
            id: "HAUNT_MOVE",
            follow_up: SWIPE,
        },
    ],
    initial: HAUNT,
};

/// `HauntedShip`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HauntedShip;

#[sts2_macros::hooks]
impl Hooks for HauntedShip {
    type Me = CreatureId;
}

impl MonsterModel for HauntedShip {
    fn name(&self) -> &'static str {
        "Haunted Ship"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 67, 63)
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
            SWIPE => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 14, 13)),
            // `StompRepeat` is 3.
            STOMP => attack_players_hits(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 5, 4), 3),
            HAUNT => {
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, PowerKind::Weak, Decimal::from(3), Some(me))?;
                add_status_cards(eng, CardKind::Dazed, HAUNT_DAZED)
            }
            _ => unreachable!("not a HauntedShip move: {state}"),
        }
    }
}
