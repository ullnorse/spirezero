//! `DivineRight` (`Models.Relics/DivineRight.cs`): Regent's starter relic; gains 3 stars on
//! entering a combat room.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

pub struct DivineRight;

impl RelicModel for DivineRight {
    fn name(&self) -> &'static str {
        "Divine Right"
    }
}

#[sts2_macros::hooks]
impl Hooks for DivineRight {
    type Me = RelicId;

    fn after_room_entered(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        // Every combat's room is a `CombatRoom`: `PlayerCmd.GainStars(StarsVar(3), Owner)`.
        eng.gain_stars(Decimal::from(3))
    }
}
