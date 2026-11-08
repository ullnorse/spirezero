//! `SlingOfCourage` (`Models.Relics/SlingOfCourage.cs`): 2 Strength when an elite fight starts.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::powers::PowerKind;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;
use crate::run::RoomType;

/// `PowerVar<StrengthPower>(2m)`.
const STRENGTH: i32 = 2;

pub struct SlingOfCourage;

impl RelicModel for SlingOfCourage {
    fn name(&self) -> &'static str {
        "Sling of Courage"
    }
}

#[sts2_macros::hooks]
impl Hooks for SlingOfCourage {
    type Me = RelicId;

    fn after_room_entered(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        if eng.s.room_type != RoomType::Elite {
            return Ok(());
        }
        // Applier `Owner.Creature`, no card source.
        let owner = eng.s.player.creature;
        eng.apply_power(
            owner,
            PowerKind::Strength,
            Decimal::from(STRENGTH),
            Some(owner),
        )
    }
}
