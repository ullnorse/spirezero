//! `Gorget` (`Models.Relics/Gorget.cs`): 4 Plating at the start of every fight.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::powers::PowerKind;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `PowerVar<PlatingPower>(4m)`.
const PLATING: i32 = 4;

pub struct Gorget;

impl RelicModel for Gorget {
    fn name(&self) -> &'static str {
        "Gorget"
    }
}

#[sts2_macros::hooks]
impl Hooks for Gorget {
    type Me = RelicId;

    fn after_room_entered(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        // `room is CombatRoom`: the combat only dispatches this hook for its own room.
        let owner = eng.s.player.creature;
        eng.apply_power(
            owner,
            PowerKind::Plating,
            Decimal::from(PLATING),
            Some(owner),
        )
    }
}
