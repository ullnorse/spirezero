//! `BeetleJuice` (`Models.Potions/BeetleJuice.cs`): apply 4 Shrink to an enemy.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `RepeatVar(4)`; `DamageDecrease` (30) is only read by the description.
const REPEAT: i32 = 4;

pub struct BeetleJuice;

#[sts2_macros::hooks]
impl Hooks for BeetleJuice {
    type Me = PotionId;
}

impl PotionModel for BeetleJuice {
    fn name(&self) -> &'static str {
        "Beetle Juice"
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
        // PowerCmd.Apply<ShrinkPower>(ctx, target, Repeat.BaseValue, Owner.Creature, null).
        eng.apply_power(
            target,
            PowerKind::Shrink,
            Decimal::from(REPEAT),
            Some(owner),
        )
    }
}
