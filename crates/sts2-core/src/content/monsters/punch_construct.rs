//! `PunchConstruct` (`Models.Monsters/PunchConstruct.cs`): Ready (10 block) -> Fast Punch (2 hits,
//! 1 Frail) -> Strong Punch -> Ready, or Fast Punch first when `StartsWithFastPunch`. It enters with
//! 1 Artifact, and loses `StartingHpReduction` HP (at least 1 stays) when set.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, ValueProps};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const READY: StateIdx = 0;
const FAST_PUNCH: StateIdx = 1;
const STRONG_PUNCH: StateIdx = 2;

const STATES: &[StateDef] = &[
    StateDef::Move {
        id: "READY_MOVE",
        follow_up: FAST_PUNCH,
    },
    StateDef::Move {
        id: "FAST_PUNCH_MOVE",
        follow_up: STRONG_PUNCH,
    },
    StateDef::Move {
        id: "STRONG_PUNCH_MOVE",
        follow_up: READY,
    },
];

static MOVES_READY: MoveTable = MoveTable {
    states: STATES,
    initial: READY,
};

static MOVES_FAST_PUNCH: MoveTable = MoveTable {
    states: STATES,
    initial: FAST_PUNCH,
};

/// Per-combat fields, set by the encounter before `SetUpForCombat`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PunchConstruct {
    /// `StartsWithFastPunch`.
    pub starts_with_fast_punch: bool,
    /// `StartingHpReduction`.
    pub starting_hp_reduction: i32,
}

#[sts2_macros::hooks]
impl Hooks for PunchConstruct {
    type Me = CreatureId;
}

impl MonsterModel for PunchConstruct {
    fn name(&self) -> &'static str {
        "Punch Construct"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 60, 55)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        if self.starts_with_fast_punch {
            &MOVES_FAST_PUNCH
        } else {
            &MOVES_READY
        }
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            READY => {
                eng.gain_block(me, Decimal::from(10), ValueProps::MOVE, None)?;
                Ok(())
            }
            FAST_PUNCH => {
                // `FastPunchRepeat` is 2.
                attack_players_hits(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 6, 5), 2)?;
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, PowerKind::Frail, Decimal::ONE, Some(me))
            }
            STRONG_PUNCH => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 16, 14))
            }
            _ => unreachable!("not a PunchConstruct move: {state}"),
        }
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.apply_power(me, PowerKind::Artifact, Decimal::ONE, Some(me))?;
        if self.starting_hp_reduction > 0 {
            // `SetCurrentHpInternal(Math.Max(1, CurrentHp - StartingHpReduction))`.
            let creature = eng.s.creature_mut(me);
            creature.hp = (creature.hp - self.starting_hp_reduction).max(1);
            eng.s.recount_enemies();
        }
        Ok(())
    }
}
