//! `LiquidBronze` (`Models.Potions/LiquidBronze.cs`): the target player gains 3 Thorns.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `PowerVar<ThornsPower>(3m)`.
const THORNS: i32 = 3;

pub struct LiquidBronze;

#[sts2_macros::hooks]
impl Hooks for LiquidBronze {
    type Me = PotionId;
}

impl PotionModel for LiquidBronze {
    fn name(&self) -> &'static str {
        "Liquid Bronze"
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
            PowerKind::Thorns,
            Decimal::from(THORNS),
            Some(owner),
        )
    }
}
