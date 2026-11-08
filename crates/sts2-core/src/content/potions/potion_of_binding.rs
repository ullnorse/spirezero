//! `PotionOfBinding` (`Models.Potions/PotionOfBinding.cs`): every hittable enemy gains 1 Weak,
//! then 1 Vulnerable. The two `PowerVar`s are read crosswise in the C# (Weak gets the
//! Vulnerable amount and the reverse); both are 1.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `PowerVar<VulnerablePower>(1m)`, applied as Weak.
const WEAK: i32 = 1;
/// `PowerVar<WeakPower>(1m)`, applied as Vulnerable.
const VULNERABLE: i32 = 1;

pub struct PotionOfBinding;

#[sts2_macros::hooks]
impl Hooks for PotionOfBinding {
    type Me = PotionId;
}

impl PotionModel for PotionOfBinding {
    fn name(&self) -> &'static str {
        "Potion of Binding"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AllEnemies
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let _ = target;
        let owner = eng.s.player.creature;
        let targets = eng.hooked().hittable_enemies();
        eng.apply_power_to_all(&targets, PowerKind::Weak, Decimal::from(WEAK), Some(owner))?;
        eng.apply_power_to_all(
            &targets,
            PowerKind::Vulnerable,
            Decimal::from(VULNERABLE),
            Some(owner),
        )
    }
}
