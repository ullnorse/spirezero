//! `HeartOfIron` (`Models.Potions/HeartOfIron.cs`): the target player gains 7 Plating.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `PowerVar<PlatingPower>(7m)`.
const PLATING: i32 = 7;

pub struct HeartOfIron;

#[sts2_macros::hooks]
impl Hooks for HeartOfIron {
    type Me = PotionId;
}

impl PotionModel for HeartOfIron {
    fn name(&self) -> &'static str {
        "Heart of Iron"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let target = target.expect("AssertValidForTargetedPotion");
        let owner = eng.s.player.creature;
        // PowerCmd.Apply<...>(ctx, target, amount, Owner.Creature, null).
        eng.apply_power(
            target,
            PowerKind::Plating,
            Decimal::from(PLATING),
            Some(owner),
        )
    }
}
