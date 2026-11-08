//! `EnergyPotion` (`Models.Potions/EnergyPotion.cs`): the target player gains 2 energy.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::decimal::Decimal;

/// `EnergyVar(2)`.
const ENERGY: i32 = 2;

pub struct EnergyPotion;

#[sts2_macros::hooks]
impl Hooks for EnergyPotion {
    type Me = PotionId;
}

impl PotionModel for EnergyPotion {
    fn name(&self) -> &'static str {
        "Energy Potion"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let _ = target.expect("AssertValidForTargetedPotion");
        // PlayerCmd.GainEnergy(Energy.BaseValue, target.Player): the only player.
        eng.gain_energy(Decimal::from(ENERGY))
    }
}
