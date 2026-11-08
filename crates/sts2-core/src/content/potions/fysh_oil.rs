//! `FyshOil` (`Models.Potions/FyshOil.cs`): the target player gains 1 Strength, then 1 Dexterity.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `PowerVar<StrengthPower>(1m)`.
const STRENGTH: i32 = 1;
/// `PowerVar<DexterityPower>(1m)`.
const DEXTERITY: i32 = 1;

pub struct FyshOil;

#[sts2_macros::hooks]
impl Hooks for FyshOil {
    type Me = PotionId;
}

impl PotionModel for FyshOil {
    fn name(&self) -> &'static str {
        "Fysh Oil"
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
        eng.apply_power(
            target,
            PowerKind::Strength,
            Decimal::from(STRENGTH),
            Some(owner),
        )?;
        eng.apply_power(
            target,
            PowerKind::Dexterity,
            Decimal::from(DEXTERITY),
            Some(owner),
        )
    }
}
