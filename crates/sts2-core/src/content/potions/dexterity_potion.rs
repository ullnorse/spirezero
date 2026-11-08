//! `DexterityPotion` (`Models.Potions/DexterityPotion.cs`): the target player gains 2 Dexterity.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `PowerVar<DexterityPower>(2m)`.
const DEXTERITY: i32 = 2;

pub struct DexterityPotion;

#[sts2_macros::hooks]
impl Hooks for DexterityPotion {
    type Me = PotionId;
}

impl PotionModel for DexterityPotion {
    fn name(&self) -> &'static str {
        "Dexterity Potion"
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
            PowerKind::Dexterity,
            Decimal::from(DEXTERITY),
            Some(owner),
        )
    }
}
