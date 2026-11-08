//! `DataDisk` (`Models.Relics/DataDisk.cs`): 1 Focus when a combat room is entered.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::powers::PowerKind;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `PowerVar<FocusPower>(1m)`.
const FOCUS: i32 = 1;

pub struct DataDisk;

impl RelicModel for DataDisk {
    fn name(&self) -> &'static str {
        "Data Disk"
    }
}

#[sts2_macros::hooks]
impl Hooks for DataDisk {
    type Me = RelicId;

    /// `AfterRoomEntered` of a `CombatRoom` (the engine only dispatches it for fights).
    fn after_room_entered(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        let owner = eng.s.player.creature;
        // No card source.
        eng.apply_power(owner, PowerKind::Focus, Decimal::from(FOCUS), Some(owner))
    }
}
