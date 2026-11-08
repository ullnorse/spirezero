//! `PowderedDemise` (`Models.Potions/PowderedDemise.cs`): the target enemy gains 9 Demise.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `DynamicVar("Demise", 9m)`.
const DEMISE: i32 = 9;

pub struct PowderedDemise;

#[sts2_macros::hooks]
impl Hooks for PowderedDemise {
    type Me = PotionId;
}

impl PotionModel for PowderedDemise {
    fn name(&self) -> &'static str {
        "Powdered Demise"
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
        eng.apply_power(
            target,
            PowerKind::Demise,
            Decimal::from(DEMISE),
            Some(owner),
        )
    }
}
