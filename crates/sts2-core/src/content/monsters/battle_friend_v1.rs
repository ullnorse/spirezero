//! `BattleFriendV1` (`Models.Monsters/BattleFriendV1.cs`): the dummy of `BattlewornDummy`'s setting 1.
//! 75 HP and one move that does nothing; it gets a 3-turn `BattlewornDummyTimeLimitPower` when it
//! joins the room. The skin is cosmetic.

use super::MonsterModel;
use super::machine::{MoveTable, StateDef, StateIdx};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `MinInitialHp = MaxInitialHp`.
const INITIAL_HP: i32 = 75;
/// `PowerCmd.Apply<BattlewornDummyTimeLimitPower>(.., 3m, ..)`.
const TIME_LIMIT: i32 = 3;

static MOVES: MoveTable = MoveTable {
    states: &[StateDef::Move {
        id: "NOTHING_MOVE",
        follow_up: 0,
    }],
    initial: 0,
};

/// `BattleFriendV1`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BattleFriendV1;

#[sts2_macros::hooks]
impl Hooks for BattleFriendV1 {
    type Me = CreatureId;
}

impl MonsterModel for BattleFriendV1 {
    fn name(&self) -> &'static str {
        "Battle Friend V1.0"
    }

    fn min_initial_hp(&self, _ascension: u8) -> i32 {
        INITIAL_HP
    }

    fn max_initial_hp(&self, _ascension: u8) -> i32 {
        INITIAL_HP
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, _eng: &mut Engine<'_>, _me: CreatureId, _state: StateIdx) -> Flow {
        Ok(())
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        let amount = Decimal::from(TIME_LIMIT);
        eng.apply_power(me, PowerKind::BattlewornDummyTimeLimit, amount, None)
    }
}
