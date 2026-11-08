//! `EyeWithTeeth` (`Models.Monsters/EyeWithTeeth.cs`): Fogmog's illusion, 6 HP. Gains
//! `IllusionPower` (and with it `MinionPower`) when added to the room, then repeats Distract:
//! 3 `Dazed` into each target's discard pile.

use super::MonsterModel;
use super::machine::{MoveTable, StateDef, StateIdx};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PileType};
use crate::content::cards::CardKind;
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const DISTRACT: StateIdx = 0;

/// `_distractAmount`.
const DISTRACT_AMOUNT: u32 = 3;

static MOVES: MoveTable = MoveTable {
    states: &[StateDef::Move {
        id: "DISTRACT_MOVE",
        follow_up: DISTRACT,
    }],
    initial: DISTRACT,
};

/// `EyeWithTeeth`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct EyeWithTeeth;

#[sts2_macros::hooks]
impl Hooks for EyeWithTeeth {
    type Me = CreatureId;
}

impl MonsterModel for EyeWithTeeth {
    fn name(&self) -> &'static str {
        "Eye with Teeth"
    }

    fn min_initial_hp(&self, _ascension: u8) -> i32 {
        6
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.apply_power(me, PowerKind::Illusion, Decimal::ONE, Some(me))
    }

    fn perform_move(&self, eng: &mut Engine<'_>, _me: CreatureId, state: StateIdx) -> Flow {
        assert_eq!(state, DISTRACT, "not an EyeWithTeeth move: {state}");
        for target in eng.s.player_creatures() {
            eng.add_to_combat_and_preview(
                target,
                CardKind::Dazed,
                PileType::Discard,
                DISTRACT_AMOUNT,
                None,
            )?;
        }
        Ok(())
    }
}
