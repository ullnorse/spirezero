//! `VineShambler` (`Models.Monsters/VineShambler.cs`): cycles Swipe (2 hits), Grasping Vines
//! (attack, then 1 `TangledPower` on each target) and Chomp, opening with Swipe.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES};
use super::{attack_players, attack_players_hits, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const GRASPING_VINES: StateIdx = 0;
const SWIPE: StateIdx = 1;
const CHOMP: StateIdx = 2;

/// `_swipeRepeat`.
const SWIPE_REPEAT: u32 = 2;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "GRASPING_VINES_MOVE",
            follow_up: CHOMP,
        },
        StateDef::Move {
            id: "SWIPE_MOVE",
            follow_up: GRASPING_VINES,
        },
        StateDef::Move {
            id: "CHOMP_MOVE",
            follow_up: SWIPE,
        },
    ],
    initial: SWIPE,
};

/// `VineShambler`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VineShambler;

#[sts2_macros::hooks]
impl Hooks for VineShambler {
    type Me = CreatureId;
}

impl MonsterModel for VineShambler {
    fn name(&self) -> &'static str {
        "Vine Shambler"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 64, 61)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let asc = eng.s.ascension;
        match state {
            GRASPING_VINES => {
                attack_players(eng, me, if_ascension(asc, DEADLY_ENEMIES, 9, 8))?;
                // PowerCmd.Apply(targets, ...): the move's targets, the `PlayerCreatures`.
                for target in eng.s.player_creatures() {
                    eng.apply_power(target, PowerKind::Tangled, Decimal::ONE, Some(me))?;
                }
                Ok(())
            }
            SWIPE => {
                let damage = if_ascension(asc, DEADLY_ENEMIES, 7, 6);
                attack_players_hits(eng, me, damage, SWIPE_REPEAT)
            }
            CHOMP => attack_players(eng, me, if_ascension(asc, DEADLY_ENEMIES, 18, 16)),
            _ => unreachable!("not a VineShambler move: {state}"),
        }
    }
}
