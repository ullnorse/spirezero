//! `SwordOfJade` (`Models.Relics/SwordOfJade.cs`): 3 Strength at the start of every fight
//! (`SwordOfStone` turns into it).

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::powers::PowerKind;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `PowerVar<StrengthPower>(3m)`.
const STRENGTH: i32 = 3;

pub struct SwordOfJade;

impl RelicModel for SwordOfJade {
    fn name(&self) -> &'static str {
        "Sword of Jade"
    }
}

#[sts2_macros::hooks]
impl Hooks for SwordOfJade {
    type Me = RelicId;

    fn after_room_entered(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        // `room is CombatRoom`: the combat only dispatches this hook for its own room. No
        // applier.
        let owner = eng.s.player.creature;
        eng.apply_power(owner, PowerKind::Strength, Decimal::from(STRENGTH), None)
    }
}
