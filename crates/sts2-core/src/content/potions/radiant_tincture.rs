//! `RadiantTincture` (`Models.Potions/RadiantTincture.cs`): the target player gains 1 energy,
//! then 3 Radiance (1 energy after each of the next 3 energy resets).

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `EnergyVar(1)`.
const ENERGY: i32 = 1;
/// `PowerVar<RadiancePower>(3m)`.
const RADIANCE: i32 = 3;

pub struct RadiantTincture;

#[sts2_macros::hooks]
impl Hooks for RadiantTincture {
    type Me = PotionId;
}

impl PotionModel for RadiantTincture {
    fn name(&self) -> &'static str {
        "Radiant Tincture"
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
        eng.gain_energy(Decimal::from(ENERGY))?;
        eng.apply_power(
            target,
            PowerKind::Radiance,
            Decimal::from(RADIANCE),
            Some(owner),
        )
    }
}
