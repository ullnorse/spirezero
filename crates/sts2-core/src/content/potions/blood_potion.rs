//! `BloodPotion` (`Models.Potions/BloodPotion.cs`): the target player heals 20% of their max HP.
//! Usable any time; it cannot be generated in combat (no in-scope combat generator draws potions).

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::decimal::Decimal;
use crate::run::{RunError, RunState};

pub struct BloodPotion;

#[sts2_macros::hooks]
impl Hooks for BloodPotion {
    type Me = PotionId;
}

/// `(decimal)maxHp * HealPercent / 100m` with `DynamicVar("HealPercent", 20m)`: 20 / 100 is
/// exactly `0.2m`, so the product is the same decimal.
fn heal_amount(max_hp: i32) -> Decimal {
    Decimal::from(max_hp) * Decimal::new(2, 1)
}

impl PotionModel for BloodPotion {
    fn name(&self) -> &'static str {
        "Blood Potion"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::AnyTime
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let target = target.expect("AssertValidForTargetedPotion");
        let max_hp = eng.s.creature(target).max_hp;
        eng.heal(target, heal_amount(max_hp))
    }

    fn on_use_out_of_combat(&self, s: &mut RunState) -> Result<(), RunError> {
        s.heal(heal_amount(s.max_hp));
        Ok(())
    }
}
