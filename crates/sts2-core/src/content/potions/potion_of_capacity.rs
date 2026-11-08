//! `PotionOfCapacity` (`Models.Potions/PotionOfCapacity.cs`): the target player gains 2 orb slots.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};

/// `RepeatVar(2)`.
const REPEAT: i32 = 2;

pub struct PotionOfCapacity;

#[sts2_macros::hooks]
impl Hooks for PotionOfCapacity {
    type Me = PotionId;
}

impl PotionModel for PotionOfCapacity {
    fn name(&self) -> &'static str {
        "Potion of Capacity"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let _ = target.expect("AssertValidForTargetedPotion");
        eng.add_orb_slots(REPEAT);
        Ok(())
    }
}
