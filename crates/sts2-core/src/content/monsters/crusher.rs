//! `Crusher` (`Models.Monsters/Crusher.cs`): the `KaiserCrabBoss`'s left arm. Added to the room
//! with `BackAttackLeftPower` and `CrabRagePower`. A fixed cycle: Thrash (attack) -> Enlarging
//! Strike (attack) -> Bug Sting (two hits, then 2 Weak and 2 Frail) -> Adapt (Strength) ->
//! Guarded Strike (attack, then 18 block) -> Thrash. `AfterCurrentHpChanged`, `BeforeDeath` and the
//! background node only drive animations and music.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES};
use super::{attack_players, attack_players_hits, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, ValueProps};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const THRASH: StateIdx = 0;
const ENLARGING_STRIKE: StateIdx = 1;
const BUG_STING: StateIdx = 2;
const ADAPT: StateIdx = 3;
const GUARDED_STRIKE: StateIdx = 4;

/// `BugStingTimes`.
const BUG_STING_TIMES: u32 = 2;
/// Bug Sting's Weak and Frail.
const BUG_STING_DEBUFF: i32 = 2;
/// Guarded Strike's block.
const GUARDED_STRIKE_BLOCK: i32 = 18;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "THRASH_MOVE",
            follow_up: ENLARGING_STRIKE,
        },
        StateDef::Move {
            id: "ENLARGING_STRIKE_MOVE",
            follow_up: BUG_STING,
        },
        StateDef::Move {
            id: "BUG_STING_MOVE",
            follow_up: ADAPT,
        },
        StateDef::Move {
            id: "ADAPT_MOVE",
            follow_up: GUARDED_STRIKE,
        },
        StateDef::Move {
            id: "GUARDED_STRIKE_MOVE",
            follow_up: THRASH,
        },
    ],
    initial: THRASH,
};

/// `Crusher`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Crusher;

#[sts2_macros::hooks]
impl Hooks for Crusher {
    type Me = CreatureId;
}

impl MonsterModel for Crusher {
    fn name(&self) -> &'static str {
        "Crusher"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 219, 209)
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
            THRASH => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 14, 12)),
            ENLARGING_STRIKE => attack_players(eng, me, 4),
            BUG_STING => {
                let damage = if_ascension(ascension, DEADLY_ENEMIES, 7, 6);
                attack_players_hits(eng, me, damage, BUG_STING_TIMES)?;
                let targets = eng.s.player_creatures();
                let amount = Decimal::from(BUG_STING_DEBUFF);
                eng.apply_power_to_all(&targets, PowerKind::Weak, amount, Some(me))?;
                eng.apply_power_to_all(&targets, PowerKind::Frail, amount, Some(me))
            }
            ADAPT => {
                let amount = if_ascension(ascension, DEADLY_ENEMIES, 3, 2);
                eng.apply_power(me, PowerKind::Strength, Decimal::from(amount), Some(me))
            }
            GUARDED_STRIKE => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 14, 12))?;
                let block = Decimal::from(GUARDED_STRIKE_BLOCK);
                eng.gain_block(me, block, ValueProps::MOVE, None)?;
                Ok(())
            }
            _ => unreachable!("not a Crusher move: {state}"),
        }
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.apply_power(me, PowerKind::BackAttackLeft, Decimal::ONE, Some(me))?;
        eng.apply_power(me, PowerKind::CrabRage, Decimal::ONE, Some(me))
    }
}
