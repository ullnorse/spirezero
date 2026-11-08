//! `BronzeScales` (`Models.Relics/BronzeScales.cs`): 3 Thorns at the start of every fight.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::powers::PowerKind;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `PowerVar<ThornsPower>(3m)`.
const THORNS: i32 = 3;

pub struct BronzeScales;

impl RelicModel for BronzeScales {
    fn name(&self) -> &'static str {
        "Bronze Scales"
    }
}

#[sts2_macros::hooks]
impl Hooks for BronzeScales {
    type Me = RelicId;

    fn after_room_entered(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        // `room is CombatRoom`: the combat only dispatches this hook for its own room.
        let owner = eng.s.player.creature;
        eng.apply_power(owner, PowerKind::Thorns, Decimal::from(THORNS), Some(owner))
    }
}
