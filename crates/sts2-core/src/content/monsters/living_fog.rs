//! `LivingFog` (`Models.Monsters/LivingFog.cs`): Advanced Gas (attack, applies `Smoggy`) -> Bloat
//! (summons a `GasBomb` into the next free slot, then attacks) -> Super Gas Blast (attack) -> Bloat
//! -> ... `BloatAmount` is 1; its setter has no other user.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, GasBomb, MonsterKind, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const ADVANCED_GAS: StateIdx = 0;
const SUPER_GAS_BLAST: StateIdx = 1;
const BLOAT: StateIdx = 2;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "ADVANCED_GAS_MOVE",
            follow_up: BLOAT,
        },
        StateDef::Move {
            id: "SUPER_GAS_BLAST_MOVE",
            follow_up: BLOAT,
        },
        StateDef::Move {
            id: "BLOAT_MOVE",
            follow_up: SUPER_GAS_BLAST,
        },
    ],
    initial: ADVANCED_GAS,
};

/// `_bloatAmount`.
const BLOAT_AMOUNT: u32 = 1;

/// `LivingFog`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LivingFog;

#[sts2_macros::hooks]
impl Hooks for LivingFog {
    type Me = CreatureId;
}

impl MonsterModel for LivingFog {
    fn name(&self) -> &'static str {
        "Living Fog"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 82, 80)
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
            ADVANCED_GAS => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 9, 8))?;
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, PowerKind::Smoggy, Decimal::ONE, Some(me))
            }
            BLOAT => {
                for _ in 0..BLOAT_AMOUNT {
                    if let Some(slot) = eng.s.encounter.next_slot(eng.s) {
                        eng.add_enemy(MonsterKind::GasBomb(GasBomb), Some(slot))?;
                    }
                }
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 6, 5))
            }
            SUPER_GAS_BLAST => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 9, 8))
            }
            _ => unreachable!("not a LivingFog move: {state}"),
        }
    }
}
