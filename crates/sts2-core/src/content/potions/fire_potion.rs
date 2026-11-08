//! `FirePotion` (`Models.Potions/FirePotion.cs`): deal 20 unpowered damage to an enemy.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId, ValueProps};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::decimal::Decimal;

/// `DamageVar(20m, ValueProp.Unpowered)`.
const DAMAGE: i32 = 20;

pub struct FirePotion;

#[sts2_macros::hooks]
impl Hooks for FirePotion {
    type Me = PotionId;
}

impl PotionModel for FirePotion {
    fn name(&self) -> &'static str {
        "Fire Potion"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let target = target.expect("AssertValidForTargetedPotion");
        // CreatureCmd.Damage(ctx, target, damage, props, Owner.Creature, null, null).
        let dealer = eng.s.player.creature;
        eng.damage(
            &[target],
            Decimal::from(DAMAGE),
            ValueProps::UNPOWERED,
            Some(dealer),
            None,
        )?;
        Ok(())
    }
}
