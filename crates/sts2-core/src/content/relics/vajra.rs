//! `Vajra` (`Models.Relics/Vajra.cs`): 1 Strength at the start of every fight.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::powers::PowerKind;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `PowerVar<StrengthPower>(1m)`.
const STRENGTH: i32 = 1;

pub struct Vajra;

impl RelicModel for Vajra {
    fn name(&self) -> &'static str {
        "Vajra"
    }
}

#[sts2_macros::hooks]
impl Hooks for Vajra {
    type Me = RelicId;

    fn after_room_entered(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        // `room is CombatRoom`: the combat only dispatches this hook for its own room.
        let owner = eng.s.player.creature;
        eng.apply_power(
            owner,
            PowerKind::Strength,
            Decimal::from(STRENGTH),
            Some(owner),
        )
    }
}
