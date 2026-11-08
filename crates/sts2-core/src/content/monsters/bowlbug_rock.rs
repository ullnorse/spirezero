//! `BowlbugRock` (`Models.Monsters/BowlbugRock.cs`): Headbutt forever, with `Imbalanced`: when a
//! Headbutt is fully blocked, `ImbalancedPower` sets `IsOffBalance`, and the Headbutt then stuns
//! its owner (`STUNNED`, then Headbutt again). `Stun` passes `DizzyMove`, which clears
//! `IsOffBalance` when the `STUNNED` move is performed; [`BowlbugRock::stun`] clears it when the
//! stun is set instead, which no one can tell apart: the flag is only set during a Headbutt, only
//! read right after one, and a pending `STUNNED` move cannot be replaced. `DIZZY_MOVE` (reached
//! from the `POST_HEADBUTT` branch, which `Stun` pre-empts) is kept for fidelity.

use super::machine::{MoveTable, StateDef, StateIdx, when};
use super::{
    DEADLY_ENEMIES, MonsterKind, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension, stun,
    update_kind,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const HEADBUTT: StateIdx = 0;
const DIZZY: StateIdx = 1;
const POST_HEADBUTT: StateIdx = 2;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "HEADBUTT_MOVE",
            follow_up: POST_HEADBUTT,
        },
        StateDef::Move {
            id: "DIZZY_MOVE",
            follow_up: HEADBUTT,
        },
        StateDef::Conditional {
            id: "POST_HEADBUTT",
            branches: &[when(DIZZY, is_off_balance), when(HEADBUTT, is_balanced)],
        },
    ],
    initial: HEADBUTT,
};

/// Per-combat fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BowlbugRock {
    /// `IsOffBalance`.
    pub is_off_balance: bool,
}

fn off_balance(s: &CombatState, id: CreatureId) -> bool {
    match s.creature(id).monster.as_ref().map(|m| m.kind) {
        Some(MonsterKind::BowlbugRock(r)) => r.is_off_balance,
        _ => unreachable!("the owner is a BowlbugRock"),
    }
}

fn is_off_balance(s: &CombatState, id: CreatureId) -> bool {
    off_balance(s, id)
}

fn is_balanced(s: &CombatState, id: CreatureId) -> bool {
    !off_balance(s, id)
}

/// Sets `IsOffBalance` on the rock `id`.
fn write_off_balance(eng: &mut Engine<'_>, id: CreatureId, value: bool) {
    update_kind(eng, id, |kind| match kind {
        MonsterKind::BowlbugRock(r) => r.is_off_balance = value,
        _ => unreachable!("the owner is a BowlbugRock"),
    });
}

impl BowlbugRock {
    /// `ImbalancedPower`'s `bowlbugRock.IsOffBalance = true`.
    pub(crate) fn set_off_balance(eng: &mut Engine<'_>, id: CreatureId) {
        write_off_balance(eng, id, true);
    }

    /// `Stun()`: `CreatureCmd.Stun(Creature, DizzyMove)`, whose `IsOffBalance = false` runs here.
    fn stun(eng: &mut Engine<'_>, me: CreatureId) {
        write_off_balance(eng, me, false);
        stun(eng, me, None);
    }
}

#[sts2_macros::hooks]
impl Hooks for BowlbugRock {
    type Me = CreatureId;
}

impl MonsterModel for BowlbugRock {
    fn name(&self) -> &'static str {
        "Bowlbug Rock"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 46, 45)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 49, 48)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            HEADBUTT => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 16, 15))?;
                if off_balance(eng.s, me) {
                    Self::stun(eng, me);
                }
                Ok(())
            }
            // `DizzyMove`: `IsOffBalance = false`.
            DIZZY => {
                write_off_balance(eng, me, false);
                Ok(())
            }
            _ => unreachable!("not a BowlbugRock move: {state}"),
        }
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.apply_power(me, PowerKind::Imbalanced, Decimal::ONE, Some(me))
    }
}
