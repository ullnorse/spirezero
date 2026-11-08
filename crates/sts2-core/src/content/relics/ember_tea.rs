//! `EmberTea` (`Models.Relics/EmberTea.cs`): 2 Strength when each of the next 5 combats starts
//! (`CombatsLeft`, 5, a `[SavedProperty]`).

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::powers::PowerKind;
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `PowerVar<StrengthPower>(2m)`.
const STRENGTH: i32 = 2;

pub struct EmberTea;

/// `EmberTea`'s fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Data {
    /// `CombatsLeft`.
    pub combats_left: i32,
}

impl RelicModel for EmberTea {
    fn name(&self) -> &'static str {
        "Ember Tea"
    }

    /// `_combatsLeft = 5`.
    fn init_data(&self) -> RelicData {
        RelicData::EmberTea(Data { combats_left: 5 })
    }
}

#[sts2_macros::hooks]
impl Hooks for EmberTea {
    type Me = RelicId;

    /// `AfterRoomEntered` of a `CombatRoom` (the engine only dispatches it for fights).
    fn after_room_entered(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        let RelicData::EmberTea(data) = eng.s.relic(me).data else {
            unreachable!("an EmberTea has EmberTea data");
        };
        if data.combats_left <= 0 {
            return Ok(());
        }
        // `CombatsLeft--` follows the power application.
        let owner = eng.s.player.creature;
        // No applier, no card source.
        eng.apply_power(owner, PowerKind::Strength, Decimal::from(STRENGTH), None)?;
        let RelicData::EmberTea(data) = &mut eng.s.relic_mut(me).data else {
            unreachable!("an EmberTea has EmberTea data");
        };
        data.combats_left -= 1;
        Ok(())
    }
}
