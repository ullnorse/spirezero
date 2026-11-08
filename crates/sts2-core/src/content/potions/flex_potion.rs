//! `FlexPotion` (`Models.Potions/FlexPotion.cs`): the target player gains 5 Strength until the end of the turn (`FlexPotionPower`).

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `PowerVar<StrengthPower>(5m)`.
const STRENGTH: i32 = 5;

pub struct FlexPotion;

#[sts2_macros::hooks]
impl Hooks for FlexPotion {
    type Me = PotionId;
}

impl PotionModel for FlexPotion {
    fn name(&self) -> &'static str {
        "Flex Potion"
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
            PowerKind::FlexPotion,
            Decimal::from(STRENGTH),
            Some(owner),
        )
    }
}
