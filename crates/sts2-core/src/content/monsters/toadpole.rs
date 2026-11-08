//! `Toadpole` (`Models.Monsters/Toadpole.cs`): Whirl -> Spiken (2 Thorns) -> Spike Spit (loses the
//! 2 Thorns, 3 hits) -> Whirl. The one in front opens with Spiken, the other with Whirl.

use super::machine::{MoveTable, StateDef, StateIdx, when};
use super::{
    DEADLY_ENEMIES, MonsterKind, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits,
    if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const INIT: StateIdx = 0;
const SPIKEN: StateIdx = 1;
const SPIKE_SPIT: StateIdx = 2;
const WHIRL: StateIdx = 3;

/// `SpikenAmount`.
const SPIKEN_AMOUNT: i32 = 2;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Conditional {
            id: "INIT_MOVE",
            branches: &[when(WHIRL, is_not_front), when(SPIKEN, is_front)],
        },
        StateDef::Move {
            id: "SPIKEN_MOVE",
            follow_up: SPIKE_SPIT,
        },
        StateDef::Move {
            id: "SPIKE_SPIT_MOVE",
            follow_up: WHIRL,
        },
        StateDef::Move {
            id: "WHIRL_MOVE",
            follow_up: SPIKEN,
        },
    ],
    initial: INIT,
};

/// Per-combat fields, set by the encounter before `SetUpForCombat`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Toadpole {
    /// `IsFront`.
    pub is_front: bool,
}

fn is_front_of(s: &CombatState, id: CreatureId) -> bool {
    match s.creature(id).monster.as_ref().map(|m| m.kind) {
        Some(MonsterKind::Toadpole(t)) => t.is_front,
        _ => unreachable!("the owner is a Toadpole"),
    }
}

fn is_front(s: &CombatState, id: CreatureId) -> bool {
    is_front_of(s, id)
}

fn is_not_front(s: &CombatState, id: CreatureId) -> bool {
    !is_front_of(s, id)
}

#[sts2_macros::hooks]
impl Hooks for Toadpole {
    type Me = CreatureId;
}

impl MonsterModel for Toadpole {
    fn name(&self) -> &'static str {
        "Toadpole"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 22, 21)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 26, 25)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            SPIKEN => eng.apply_power(
                me,
                PowerKind::Thorns,
                Decimal::from(SPIKEN_AMOUNT),
                Some(me),
            ),
            SPIKE_SPIT => {
                eng.apply_power(
                    me,
                    PowerKind::Thorns,
                    Decimal::from(-SPIKEN_AMOUNT),
                    Some(me),
                )?;
                // `SpikeSpitRepeat` is 3.
                attack_players_hits(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 4, 3), 3)
            }
            WHIRL => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 8, 7)),
            _ => unreachable!("not a Toadpole move: {state}"),
        }
    }
}
