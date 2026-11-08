//! `PhrogParasite` (`Models.Monsters/PhrogParasite.cs`): gains 1 Infested (amount 4) when added to
//! the room. Infect (3 `Infection` into the discard pile) and Lash (4 hits), alternating, Infect
//! first. The `RAND` branch has no incoming transition, so it is left out.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, add_status_cards, attack_players_hits,
    if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::cards::CardKind;
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `_lashRepeat`.
const LASH_REPEAT: u32 = 4;
/// The `Infection` cards Infect adds.
const INFECT_COUNT: u32 = 3;
/// The amount of the Infested power.
const INFESTED_AMOUNT: i32 = 4;

const INFECT: StateIdx = 0;
const LASH: StateIdx = 1;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "INFECT_MOVE",
            follow_up: LASH,
        },
        StateDef::Move {
            id: "LASH_MOVE",
            follow_up: INFECT,
        },
    ],
    initial: INFECT,
};

/// `PhrogParasite`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PhrogParasite;

#[sts2_macros::hooks]
impl Hooks for PhrogParasite {
    type Me = CreatureId;
}

impl MonsterModel for PhrogParasite {
    fn name(&self) -> &'static str {
        "Phrog Parasite"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 66, 61)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 68, 64)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        match state {
            INFECT => add_status_cards(eng, CardKind::Infection, INFECT_COUNT),
            LASH => {
                let damage = if_ascension(eng.s.ascension, DEADLY_ENEMIES, 5, 4);
                attack_players_hits(eng, me, damage, LASH_REPEAT)
            }
            _ => unreachable!("not a PhrogParasite move: {state}"),
        }
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.apply_power(
            me,
            PowerKind::Infested,
            Decimal::from(INFESTED_AMOUNT),
            Some(me),
        )
    }
}
