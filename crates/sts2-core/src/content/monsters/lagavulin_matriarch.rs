//! `LagavulinMatriarch` (`Models.Monsters/LagavulinMatriarch.cs`): enters asleep with 12 `Plating`
//! and 3 `Asleep`, doing nothing (`SLEEP_MOVE`) while `AsleepPower` lasts; damage or the power
//! running out wakes it into Slash -> Disembowel (2 hits) -> Slash 2 (attack and block) -> Soul
//! Siphon (-2 Strength and Dexterity on the players, +2 Strength) -> Slash. `IsAwake`,
//! `IsShellAwake` (half HP), the sleeping VFX and the wake-up move are animation only.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow, ValueProps};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const SLEEP_BRANCH: StateIdx = 0;
const SLEEP: StateIdx = 1;
const SLASH: StateIdx = 2;
const SLASH_2: StateIdx = 3;
const SOUL_SIPHON: StateIdx = 4;
const DISEMBOWEL: StateIdx = 5;

fn is_asleep(s: &CombatState, me: CreatureId) -> bool {
    s.creature(me).has_power(PowerKind::Asleep)
}

fn is_awake(s: &CombatState, me: CreatureId) -> bool {
    !is_asleep(s, me)
}

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Conditional {
            id: "SLEEP_BRANCH",
            branches: &[(SLEEP, is_asleep), (SLASH, is_awake)],
        },
        StateDef::Move {
            id: "SLEEP_MOVE",
            follow_up: SLEEP_BRANCH,
        },
        StateDef::Move {
            id: "SLASH_MOVE",
            follow_up: DISEMBOWEL,
        },
        StateDef::Move {
            id: "SLASH2_MOVE",
            follow_up: SOUL_SIPHON,
        },
        StateDef::Move {
            id: "SOUL_SIPHON_MOVE",
            follow_up: SLASH,
        },
        StateDef::Move {
            id: "DISEMBOWEL_MOVE",
            follow_up: SLASH_2,
        },
    ],
    initial: SLEEP,
};

/// `LagavulinMatriarch`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LagavulinMatriarch;

impl LagavulinMatriarch {
    /// The state `AsleepPower` stuns its owner into (`SLASH_MOVE`).
    pub const SLASH: StateIdx = SLASH;
}

#[sts2_macros::hooks]
impl Hooks for LagavulinMatriarch {
    type Me = CreatureId;
}

impl MonsterModel for LagavulinMatriarch {
    fn name(&self) -> &'static str {
        "Lagavulin Matriarch"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 233, 222)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    /// `Sleep`: `Plating` 12 and `Asleep` 3.
    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.apply_power(me, PowerKind::Plating, Decimal::from(12), Some(me))?;
        eng.apply_power(me, PowerKind::Asleep, Decimal::from(3), Some(me))
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            SLEEP => Ok(()),
            SLASH => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 21, 19)),
            SLASH_2 => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 14, 12))?;
                let block = if_ascension(ascension, TOUGH_ENEMIES, 14, 12);
                eng.gain_block(me, Decimal::from(block), ValueProps::MOVE, None)?;
                Ok(())
            }
            // `DisembowelRepeat` 2.
            DISEMBOWEL => {
                attack_players_hits(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 10, 9), 2)
            }
            SOUL_SIPHON => {
                let targets = eng.s.player_creatures();
                let minus_two = Decimal::from(-2);
                eng.apply_power_to_all(&targets, PowerKind::Strength, minus_two, Some(me))?;
                eng.apply_power_to_all(&targets, PowerKind::Dexterity, minus_two, Some(me))?;
                eng.apply_power(me, PowerKind::Strength, Decimal::from(2), Some(me))
            }
            _ => unreachable!("not a LagavulinMatriarch move: {state}"),
        }
    }
}
