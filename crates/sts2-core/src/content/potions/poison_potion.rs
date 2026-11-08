//! `PoisonPotion` (`Models.Potions/PoisonPotion.cs`): the target enemy gains 6 Poison.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `PowerVar<PoisonPower>(6m)`.
const POISON: i32 = 6;

pub struct PoisonPotion;

#[sts2_macros::hooks]
impl Hooks for PoisonPotion {
    type Me = PotionId;
}

impl PotionModel for PoisonPotion {
    fn name(&self) -> &'static str {
        "Poison Potion"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let target = target.expect("AssertValidForTargetedPotion");
        let owner = eng.s.player.creature;
        eng.apply_power(
            target,
            PowerKind::Poison,
            Decimal::from(POISON),
            Some(owner),
        )
    }
}
