//! `SoulNexus` (`Models.Monsters/SoulNexus.cs`): Soul Burn (attack) first, then a random branch
//! over Soul Burn, Maelstrom (4 hits) and Drain Life (attack, 2 Vulnerable, 2 Weak), none twice
//! in a row. The death animation hooks are cosmetic.

use super::machine::{Branch, MoveTable, Repeat, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const SOUL_BURN: StateIdx = 0;
const MAELSTROM: StateIdx = 1;
const DRAIN_LIFE: StateIdx = 2;
const RAND: StateIdx = 3;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "SOUL_BURN_MOVE",
            follow_up: RAND,
        },
        StateDef::Move {
            id: "MAELSTROM_MOVE",
            follow_up: RAND,
        },
        StateDef::Move {
            id: "DRAIN_LIFE_MOVE",
            follow_up: RAND,
        },
        StateDef::Random {
            id: "RAND",
            branches: &[
                Branch::weighted(SOUL_BURN, Repeat::CannotRepeat, 1.0),
                Branch::weighted(MAELSTROM, Repeat::CannotRepeat, 1.0),
                Branch::weighted(DRAIN_LIFE, Repeat::CannotRepeat, 1.0),
            ],
        },
    ],
    initial: SOUL_BURN,
};

/// `SoulNexus`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SoulNexus;

#[sts2_macros::hooks]
impl Hooks for SoulNexus {
    type Me = CreatureId;
}

impl MonsterModel for SoulNexus {
    fn name(&self) -> &'static str {
        "Soul Nexus"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 254, 234)
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
            SOUL_BURN => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 31, 29)),
            MAELSTROM => {
                attack_players_hits(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 7, 6), 4)
            }
            DRAIN_LIFE => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 19, 18))?;
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(
                    &targets,
                    PowerKind::Vulnerable,
                    Decimal::from(2),
                    Some(me),
                )?;
                eng.apply_power_to_all(&targets, PowerKind::Weak, Decimal::from(2), Some(me))
            }
            _ => unreachable!("not a SoulNexus move: {state}"),
        }
    }
}
