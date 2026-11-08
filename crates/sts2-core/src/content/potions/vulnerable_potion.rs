//! `VulnerablePotion` (`Models.Potions/VulnerablePotion.cs`): apply 3 Vulnerable to an enemy.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `PowerVar<VulnerablePower>(3m)`.
const VULNERABLE: i32 = 3;

pub struct VulnerablePotion;

#[sts2_macros::hooks]
impl Hooks for VulnerablePotion {
    type Me = PotionId;
}

impl PotionModel for VulnerablePotion {
    fn name(&self) -> &'static str {
        "Vulnerable Potion"
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
        // PowerCmd.Apply<VulnerablePower>(ctx, target, Vulnerable.BaseValue, Owner.Creature, null).
        eng.apply_power(
            target,
            PowerKind::Vulnerable,
            Decimal::from(VULNERABLE),
            Some(owner),
        )
    }
}
