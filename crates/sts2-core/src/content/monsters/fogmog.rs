//! `Fogmog` (`Models.Monsters/Fogmog.cs`): opens with Illusion, which adds an `EyeWithTeeth` in
//! the `illusion` slot; then Swipe (attack, +1 Strength), then a random branch over Swipe
//! (weight 0.4) and Headbutt (weight 0.6), neither twice in a row. The random Swipe goes on to
//! Headbutt, and Headbutt back to Swipe.

use super::machine::{Branch, MoveTable, Repeat, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, EyeWithTeeth, MonsterKind, MonsterModel, TOUGH_ENEMIES};
use super::{attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const ILLUSION: StateIdx = 0;
const SWIPE: StateIdx = 1;
const SWIPE_RANDOM: StateIdx = 2;
const BRANCH: StateIdx = 3;
const HEADBUTT: StateIdx = 4;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "ILLUSION_MOVE",
            follow_up: SWIPE,
        },
        StateDef::Move {
            id: "SWIPE_MOVE",
            follow_up: BRANCH,
        },
        StateDef::Move {
            id: "SWIPE_RANDOM_MOVE",
            follow_up: HEADBUTT,
        },
        StateDef::Random {
            id: "BRANCH",
            branches: &[
                Branch::weighted(SWIPE_RANDOM, Repeat::CannotRepeat, 0.4),
                Branch::weighted(HEADBUTT, Repeat::CannotRepeat, 0.6),
            ],
        },
        StateDef::Move {
            id: "HEADBUTT_MOVE",
            follow_up: SWIPE,
        },
    ],
    initial: ILLUSION,
};

/// `FogmogNormal.illusionSlot`.
const ILLUSION_SLOT: &str = "illusion";

/// `Fogmog`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Fogmog;

#[sts2_macros::hooks]
impl Hooks for Fogmog {
    type Me = CreatureId;
}

impl MonsterModel for Fogmog {
    fn name(&self) -> &'static str {
        "Fogmog"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 78, 74)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        match state {
            // `IsLiveCombat` is always true for a real combat.
            ILLUSION => {
                let eye = MonsterKind::EyeWithTeeth(EyeWithTeeth);
                eng.add_enemy(eye, Some(ILLUSION_SLOT))?;
                Ok(())
            }
            SWIPE | SWIPE_RANDOM => {
                let damage = if_ascension(eng.s.ascension, DEADLY_ENEMIES, 9, 8);
                attack_players(eng, me, damage)?;
                eng.apply_power(me, PowerKind::Strength, Decimal::ONE, Some(me))
            }
            HEADBUTT => {
                let damage = if_ascension(eng.s.ascension, DEADLY_ENEMIES, 16, 14);
                attack_players(eng, me, damage)
            }
            _ => unreachable!("not a Fogmog move: {state}"),
        }
    }
}
