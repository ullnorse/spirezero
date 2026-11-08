//! `FruitJuice` (`Models.Potions/FruitJuice.cs`): the target player gains 5 max HP (and heals as
//! much). Usable any time; it cannot be generated in combat (no in-scope combat generator draws
//! potions).

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::decimal::Decimal;
use crate::run::{RunError, RunState};

/// `MaxHpVar(5m)`.
const MAX_HP: i32 = 5;

pub struct FruitJuice;

#[sts2_macros::hooks]
impl Hooks for FruitJuice {
    type Me = PotionId;
}

impl PotionModel for FruitJuice {
    fn name(&self) -> &'static str {
        "Fruit Juice"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::AnyTime
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let target = target.expect("AssertValidForTargetedPotion");
        eng.gain_max_hp(target, Decimal::from(MAX_HP))
    }

    fn on_use_out_of_combat(&self, s: &mut RunState) -> Result<(), RunError> {
        s.gain_max_hp(MAX_HP);
        Ok(())
    }
}
