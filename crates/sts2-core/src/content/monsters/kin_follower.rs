//! `KinFollower` (`Models.Monsters/KinFollower.cs`): a Minion (secondary enemy). Quick Slash,
//! Boomerang (2 hits) and Power Dance (+Strength), in a cycle; the one with `StartsWithDance`
//! opens with Power Dance. The hair skin is picked on `Rng.Chaotic`, which nothing else reads.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `_boomerangRepeat`.
const BOOMERANG_REPEAT: u32 = 2;

const QUICK_SLASH: StateIdx = 0;
const BOOMERANG: StateIdx = 1;
const POWER_DANCE: StateIdx = 2;

const STATES: &[StateDef] = &[
    StateDef::Move {
        id: "QUICK_SLASH_MOVE",
        follow_up: BOOMERANG,
    },
    StateDef::Move {
        id: "BOOMERANG_MOVE",
        follow_up: POWER_DANCE,
    },
    StateDef::Move {
        id: "POWER_DANCE_MOVE",
        follow_up: QUICK_SLASH,
    },
];

static MOVES: MoveTable = MoveTable {
    states: STATES,
    initial: QUICK_SLASH,
};

static MOVES_DANCE: MoveTable = MoveTable {
    states: STATES,
    initial: POWER_DANCE,
};

/// Per-combat fields, set by the encounter before `SetUpForCombat`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct KinFollower {
    /// `StartsWithDance`.
    pub starts_with_dance: bool,
}

#[sts2_macros::hooks]
impl Hooks for KinFollower {
    type Me = CreatureId;
}

impl MonsterModel for KinFollower {
    fn name(&self) -> &'static str {
        "Kin Follower"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 62, 58)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 63, 59)
    }

    fn moves(&self) -> &'static MoveTable {
        if self.starts_with_dance {
            &MOVES_DANCE
        } else {
            &MOVES
        }
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            // `QuickSlashDamage` and `BoomerangDamage` are the same at every ascension.
            QUICK_SLASH => attack_players(eng, me, 5),
            BOOMERANG => attack_players_hits(eng, me, 2, BOOMERANG_REPEAT),
            POWER_DANCE => {
                let strength = if_ascension(ascension, DEADLY_ENEMIES, 3, 2);
                eng.apply_power(me, PowerKind::Strength, Decimal::from(strength), Some(me))
            }
            _ => unreachable!("not a KinFollower move: {state}"),
        }
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.apply_power(me, PowerKind::Minion, Decimal::ONE, Some(me))
    }
}
