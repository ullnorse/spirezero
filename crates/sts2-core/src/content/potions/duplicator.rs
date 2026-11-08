//! `Duplicator` (`Models.Potions/Duplicator.cs`): the target player's next card is played an extra time (`DuplicationPower`, applied by the target itself).

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// The stack applied (`1m`).
const DUPLICATION: i32 = 1;

pub struct Duplicator;

#[sts2_macros::hooks]
impl Hooks for Duplicator {
    type Me = PotionId;
}

impl PotionModel for Duplicator {
    fn name(&self) -> &'static str {
        "Duplicator"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let target = target.expect("AssertValidForTargetedPotion");
        // PowerCmd.Apply<...>(ctx, target, amount, target, null).
        eng.apply_power(
            target,
            PowerKind::Duplication,
            Decimal::from(DUPLICATION),
            Some(target),
        )
    }
}
