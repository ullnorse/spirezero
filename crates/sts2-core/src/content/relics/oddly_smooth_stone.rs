//! `OddlySmoothStone` (`Models.Relics/OddlySmoothStone.cs`): 1 Dexterity at the start of every fight.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::powers::PowerKind;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `PowerVar<DexterityPower>(1m)`.
const DEXTERITY: i32 = 1;

pub struct OddlySmoothStone;

impl RelicModel for OddlySmoothStone {
    fn name(&self) -> &'static str {
        "Oddly Smooth Stone"
    }
}

#[sts2_macros::hooks]
impl Hooks for OddlySmoothStone {
    type Me = RelicId;

    fn after_room_entered(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        // `room is CombatRoom`: the combat only dispatches this hook for its own room.
        let owner = eng.s.player.creature;
        eng.apply_power(
            owner,
            PowerKind::Dexterity,
            Decimal::from(DEXTERITY),
            Some(owner),
        )
    }
}
