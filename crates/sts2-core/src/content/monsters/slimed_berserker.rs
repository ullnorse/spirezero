//! `SlimedBerserker` (`Models.Monsters/SlimedBerserker.cs`): Vomit Ichor (10 Slimed into the
//! discard pile) -> Furious Pummeling (4 hits) -> Leeching Hug (3 Weak, +3 Strength) -> Smother
//! -> Vomit Ichor.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, add_slimed, attack_players, attack_players_hits,
    if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const VOMIT_ICHOR: StateIdx = 0;
const SMOTHER: StateIdx = 1;
const LEECHING_HUG: StateIdx = 2;
const FURIOUS_PUMMELING: StateIdx = 3;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "VOMIT_ICHOR_MOVE",
            follow_up: FURIOUS_PUMMELING,
        },
        StateDef::Move {
            id: "SMOTHER_MOVE",
            follow_up: VOMIT_ICHOR,
        },
        StateDef::Move {
            id: "LEECHING_HUG_MOVE",
            follow_up: SMOTHER,
        },
        StateDef::Move {
            id: "FURIOUS_PUMMELING_MOVE",
            follow_up: LEECHING_HUG,
        },
    ],
    initial: VOMIT_ICHOR,
};

/// `_vomitSlimeInDiscard`.
const VOMIT_SLIME: u32 = 10;

/// `SlimedBerserker`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SlimedBerserker;

#[sts2_macros::hooks]
impl Hooks for SlimedBerserker {
    type Me = CreatureId;
}

impl MonsterModel for SlimedBerserker {
    fn name(&self) -> &'static str {
        "Slimed Berserker"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 281, 261)
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
            VOMIT_ICHOR => add_slimed(eng, VOMIT_SLIME),
            LEECHING_HUG => {
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, PowerKind::Weak, Decimal::from(3), None)?;
                eng.apply_power(me, PowerKind::Strength, Decimal::from(3), Some(me))
            }
            FURIOUS_PUMMELING => {
                attack_players_hits(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 5, 4), 4)
            }
            SMOTHER => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 33, 30)),
            _ => unreachable!("not a SlimedBerserker move: {state}"),
        }
    }
}
