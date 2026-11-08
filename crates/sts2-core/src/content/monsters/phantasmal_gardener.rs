//! `PhantasmalGardener` (`Models.Monsters/PhantasmalGardener.cs`): Bite -> Lash -> Flail -> Enlarge
//! -> Bite, entering the cycle at the move its slot picks (`first` Flail, `second` Bite, `third`
//! Lash, `fourth` Enlarge). It enters with `Skittish`. `EnlargeTriggers` only scales the sprite.

use super::machine::{MoveTable, StateDef, StateIdx, when};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const BITE: StateIdx = 0;
const LASH: StateIdx = 1;
const ENLARGE: StateIdx = 2;
const FLAIL: StateIdx = 3;
const INIT: StateIdx = 4;

fn in_slot(s: &CombatState, id: CreatureId, slot: &str) -> bool {
    s.creature(id).slot == Some(slot)
}

fn is_first(s: &CombatState, id: CreatureId) -> bool {
    in_slot(s, id, "first")
}

fn is_second(s: &CombatState, id: CreatureId) -> bool {
    in_slot(s, id, "second")
}

fn is_third(s: &CombatState, id: CreatureId) -> bool {
    in_slot(s, id, "third")
}

fn is_fourth(s: &CombatState, id: CreatureId) -> bool {
    in_slot(s, id, "fourth")
}

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "BITE_MOVE",
            follow_up: LASH,
        },
        StateDef::Move {
            id: "LASH_MOVE",
            follow_up: FLAIL,
        },
        StateDef::Move {
            id: "ENLARGE_MOVE",
            follow_up: BITE,
        },
        StateDef::Move {
            id: "FLAIL_MOVE",
            follow_up: ENLARGE,
        },
        StateDef::Conditional {
            id: "INIT_MOVE",
            branches: &[
                when(FLAIL, is_first),
                when(BITE, is_second),
                when(LASH, is_third),
                when(ENLARGE, is_fourth),
            ],
        },
    ],
    initial: INIT,
};

/// `PhantasmalGardener`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PhantasmalGardener;

#[sts2_macros::hooks]
impl Hooks for PhantasmalGardener {
    type Me = CreatureId;
}

impl MonsterModel for PhantasmalGardener {
    fn name(&self) -> &'static str {
        "Phantasmal Gardener"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 27, 26)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 32, 31)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            // `BiteDamage` 5, `LashDamage` 7, `FlailDamage` 1 and `FlailRepeat` 3 at every ascension.
            BITE => attack_players(eng, me, 5),
            LASH => attack_players(eng, me, 7),
            FLAIL => attack_players_hits(eng, me, 1, 3),
            ENLARGE => {
                let strength = if_ascension(ascension, DEADLY_ENEMIES, 3, 2);
                eng.apply_power(me, PowerKind::Strength, Decimal::from(strength), Some(me))
            }
            _ => unreachable!("not a PhantasmalGardener move: {state}"),
        }
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        let amount = if_ascension(eng.s.ascension, TOUGH_ENEMIES, 7, 6);
        eng.apply_power(me, PowerKind::Skittish, Decimal::from(amount), Some(me))
    }
}
