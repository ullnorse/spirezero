//! `FocusPotion` (`Models.Potions/FocusPotion.cs`): the target player gains 2 Focus.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `PowerVar<FocusPower>(2m)`.
const FOCUS: i32 = 2;

pub struct FocusPotion;

#[sts2_macros::hooks]
impl Hooks for FocusPotion {
    type Me = PotionId;
}

impl PotionModel for FocusPotion {
    fn name(&self) -> &'static str {
        "Focus Potion"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let target = target.expect("AssertValidForTargetedPotion");
        eng.apply_power(target, PowerKind::Focus, Decimal::from(FOCUS), Some(target))
    }
}
