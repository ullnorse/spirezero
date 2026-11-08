//! `StrengthPotion` (`Models.Potions/StrengthPotion.cs`): the target player gains 2 Strength.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `PowerVar<StrengthPower>(2m)`.
const STRENGTH: i32 = 2;

pub struct StrengthPotion;

#[sts2_macros::hooks]
impl Hooks for StrengthPotion {
    type Me = PotionId;
}

impl PotionModel for StrengthPotion {
    fn name(&self) -> &'static str {
        "Strength Potion"
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
        // PowerCmd.Apply<StrengthPower>(ctx, target, Strength.BaseValue, Owner.Creature, null).
        eng.apply_power(
            target,
            PowerKind::Strength,
            Decimal::from(STRENGTH),
            Some(owner),
        )
    }
}
