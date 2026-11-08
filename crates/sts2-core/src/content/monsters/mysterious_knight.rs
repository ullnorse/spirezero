//! `MysteriousKnight` (`Models.Monsters/MysteriousKnight.cs`): a `FlailKnight` that enters with 6
//! Strength and 6 `Plating`.

use super::MonsterModel;
use super::flail_knight;
use super::machine::{MoveTable, StateIdx};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `MysteriousKnight`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MysteriousKnight;

#[sts2_macros::hooks]
impl Hooks for MysteriousKnight {
    type Me = CreatureId;
}

impl MonsterModel for MysteriousKnight {
    fn name(&self) -> &'static str {
        "Mysterious Knight"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        flail_knight::initial_hp(ascension)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        flail_knight::initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &flail_knight::MOVES
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.apply_power(me, PowerKind::Strength, Decimal::from(6), Some(me))?;
        eng.apply_power(me, PowerKind::Plating, Decimal::from(6), Some(me))
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        flail_knight::perform(eng, me, state)
    }
}
