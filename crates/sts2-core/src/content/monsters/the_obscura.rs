//! `TheObscura` (`Models.Monsters/TheObscura.cs`): opens with Illusion, which adds a `Parafright`
//! in the `illusion` slot, then a random branch over Piercing Gaze (attack), Wail (`SAIL_MOVE`:
//! 3 Strength to its whole side) and Hardening Strike (attack, then block), none twice in a row.
//! `HasSummoned` is never read.

use super::machine::{Branch, MoveTable, Repeat, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterKind, MonsterModel, Parafright, TOUGH_ENEMIES};
use super::{attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, ValueProps};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const ILLUSION: StateIdx = 0;
const PIERCING_GAZE: StateIdx = 1;
const WAIL: StateIdx = 2;
const HARDENING_STRIKE: StateIdx = 3;
const RAND: StateIdx = 4;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "ILLUSION_MOVE",
            follow_up: RAND,
        },
        StateDef::Move {
            id: "PIERCING_GAZE_MOVE",
            follow_up: RAND,
        },
        StateDef::Move {
            id: "SAIL_MOVE",
            follow_up: RAND,
        },
        StateDef::Move {
            id: "HARDENING_STRIKE_MOVE",
            follow_up: RAND,
        },
        StateDef::Random {
            id: "RAND",
            branches: &[
                Branch::new(PIERCING_GAZE, Repeat::CannotRepeat),
                Branch::new(WAIL, Repeat::CannotRepeat),
                Branch::new(HARDENING_STRIKE, Repeat::CannotRepeat),
            ],
        },
    ],
    initial: ILLUSION,
};

/// `TheObscuraNormal.illusionSlot`.
const ILLUSION_SLOT: &str = "illusion";

/// Wail's Strength.
const WAIL_STRENGTH: i32 = 3;

/// `TheObscura`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TheObscura;

#[sts2_macros::hooks]
impl Hooks for TheObscura {
    type Me = CreatureId;
}

impl MonsterModel for TheObscura {
    fn name(&self) -> &'static str {
        "The Obscura"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 129, 123)
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
            // `IsLiveCombat` is always true for a real combat.
            ILLUSION => {
                eng.add_enemy(MonsterKind::Parafright(Parafright), Some(ILLUSION_SLOT))?;
                Ok(())
            }
            PIERCING_GAZE => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 11, 10))
            }
            WAIL => {
                let side = eng.s.creature(me).side;
                let team = eng.s.creatures_on(side);
                eng.apply_power_to_all(
                    &team,
                    PowerKind::Strength,
                    Decimal::from(WAIL_STRENGTH),
                    Some(me),
                )
            }
            HARDENING_STRIKE => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 7, 6))?;
                let block = if_ascension(ascension, DEADLY_ENEMIES, 7, 6);
                eng.gain_block(me, Decimal::from(block), ValueProps::MOVE, None)?;
                Ok(())
            }
            _ => unreachable!("not a TheObscura move: {state}"),
        }
    }
}
