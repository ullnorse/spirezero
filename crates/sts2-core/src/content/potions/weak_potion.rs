//! `WeakPotion` (`Models.Potions/WeakPotion.cs`): the target enemy gains 3 Weak.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `PowerVar<WeakPower>(3m)`.
const WEAK: i32 = 3;

pub struct WeakPotion;

#[sts2_macros::hooks]
impl Hooks for WeakPotion {
    type Me = PotionId;
}

impl PotionModel for WeakPotion {
    fn name(&self) -> &'static str {
        "Weak Potion"
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
        // PowerCmd.Apply<...>(ctx, target, amount, Owner.Creature, null).
        eng.apply_power(target, PowerKind::Weak, Decimal::from(WEAK), Some(owner))
    }
}
