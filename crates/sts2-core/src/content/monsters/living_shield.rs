//! `LivingShield` (`Models.Monsters/LivingShield.cs`): starts with Rampart (25), which blocks for
//! the `TurretOperator`s. Shield Slam while another enemy lives; once alone it Smashes (damage,
//! +3 Strength) every turn.

use super::machine::{MoveTable, StateDef, StateIdx, when};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const SHIELD_SLAM: StateIdx = 0;
const SMASH: StateIdx = 1;
const SHIELD_SLAM_BRANCH: StateIdx = 2;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "SHIELD_SLAM_MOVE",
            follow_up: SHIELD_SLAM_BRANCH,
        },
        StateDef::Move {
            id: "SMASH_MOVE",
            follow_up: SMASH,
        },
        StateDef::Conditional {
            id: "SHIELD_SLAM_BRANCH",
            branches: &[when(SHIELD_SLAM, has_allies), when(SMASH, has_no_allies)],
        },
    ],
    initial: SHIELD_SLAM,
};

/// `ShieldSlamDamage`.
const SHIELD_SLAM_DAMAGE: i32 = 6;
/// `EnrageStr`.
const ENRAGE_STRENGTH: i32 = 3;
/// The `RampartPower` amount.
const RAMPART: i32 = 25;

/// `GetAllyCount`: the living teammates other than the monster itself.
fn ally_count(s: &CombatState, me: CreatureId) -> usize {
    s.enemies
        .iter()
        .filter(|&&e| e != me && s.creature(e).is_alive())
        .count()
}

fn has_allies(s: &CombatState, me: CreatureId) -> bool {
    ally_count(s, me) > 0
}

fn has_no_allies(s: &CombatState, me: CreatureId) -> bool {
    ally_count(s, me) == 0
}

/// `LivingShield`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LivingShield;

#[sts2_macros::hooks]
impl Hooks for LivingShield {
    type Me = CreatureId;
}

impl MonsterModel for LivingShield {
    fn name(&self) -> &'static str {
        "Living Shield"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 65, 55)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.apply_power(me, PowerKind::Rampart, Decimal::from(RAMPART), Some(me))
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        match state {
            SHIELD_SLAM => attack_players(eng, me, SHIELD_SLAM_DAMAGE),
            SMASH => {
                attack_players(
                    eng,
                    me,
                    if_ascension(eng.s.ascension, DEADLY_ENEMIES, 18, 16),
                )?;
                eng.apply_power(
                    me,
                    PowerKind::Strength,
                    Decimal::from(ENRAGE_STRENGTH),
                    Some(me),
                )
            }
            _ => unreachable!("not a LivingShield move: {state}"),
        }
    }
}
