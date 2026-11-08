//! `PotionShapedRock` (`Models.Potions/PotionShapedRock.cs`): deal 15 unpowered damage to an enemy.

use crate::combat::ValueProps;
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::decimal::Decimal;

/// `DamageVar(15m, ValueProp.Unpowered)`.
const DAMAGE: i32 = 15;

pub struct PotionShapedRock;

#[sts2_macros::hooks]
impl Hooks for PotionShapedRock {
    type Me = PotionId;
}

impl PotionModel for PotionShapedRock {
    fn name(&self) -> &'static str {
        "Potion-Shaped Rock"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let target = target.expect("AssertValidForTargetedPotion");
        let dealer = eng.s.player.creature;
        // CreatureCmd.Damage(ctx, target, Damage, Owner.Creature, null, null).
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
