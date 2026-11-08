//! `GhostInAJar` (`Models.Potions/GhostInAJar.cs`): the target player gains 1 Intangible.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `PowerVar<IntangiblePower>(1m)`.
const INTANGIBLE: i32 = 1;

pub struct GhostInAJar;

#[sts2_macros::hooks]
impl Hooks for GhostInAJar {
    type Me = PotionId;
}

impl PotionModel for GhostInAJar {
    fn name(&self) -> &'static str {
        "Ghost in a Jar"
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
            PowerKind::Intangible,
            Decimal::from(INTANGIBLE),
            Some(owner),
        )
    }
}
