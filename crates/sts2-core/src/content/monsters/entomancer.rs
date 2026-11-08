//! `Entomancer` (`Models.Monsters/Entomancer.cs`): enters with 1 `PersonalHive`; Bees (many small
//! hits) -> Spear -> Pheromone Spit (+1 `PersonalHive` and +1 Strength, or +2 Strength once the
//! hive is at 3) -> Bees.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const PHEROMONE_SPIT: StateIdx = 0;
const BEES: StateIdx = 1;
const SPEAR: StateIdx = 2;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "PHEROMONE_SPIT_MOVE",
            follow_up: BEES,
        },
        StateDef::Move {
            id: "BEES_MOVE",
            follow_up: SPEAR,
        },
        StateDef::Move {
            id: "SPEAR_MOVE",
            follow_up: PHEROMONE_SPIT,
        },
    ],
    initial: BEES,
};

/// `Entomancer`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Entomancer;

#[sts2_macros::hooks]
impl Hooks for Entomancer {
    type Me = CreatureId;
}

impl MonsterModel for Entomancer {
    fn name(&self) -> &'static str {
        "Entomancer"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 165, 145)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.apply_power(me, PowerKind::PersonalHive, Decimal::ONE, Some(me))
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            PHEROMONE_SPIT => {
                let hive = eng
                    .s
                    .creature(me)
                    .powers
                    .iter()
                    .find(|p| p.kind == PowerKind::PersonalHive)
                    .map(|p| p.amount);
                if hive.is_none_or(|amount| amount >= 3) {
                    return eng.apply_power(me, PowerKind::Strength, Decimal::from(2), Some(me));
                }
                eng.apply_power(me, PowerKind::PersonalHive, Decimal::ONE, Some(me))?;
                eng.apply_power(me, PowerKind::Strength, Decimal::ONE, Some(me))
            }
            BEES => {
                // `BeesDamage` 3 at every ascension, `BeesRepeat` 7 or 8.
                let repeat = if ascension >= DEADLY_ENEMIES { 8 } else { 7 };
                attack_players_hits(eng, me, 3, repeat)
            }
            SPEAR => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 20, 18)),
            _ => unreachable!("not an Entomancer move: {state}"),
        }
    }
}
