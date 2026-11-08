//! `BoneBrew` (`Models.Potions/BoneBrew.cs`): the target player summons 15.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::decimal::Decimal;

/// `SummonVar(15m)`.
const SUMMON: i32 = 15;

pub struct BoneBrew;

#[sts2_macros::hooks]
impl Hooks for BoneBrew {
    type Me = PotionId;
}

impl PotionModel for BoneBrew {
    fn name(&self) -> &'static str {
        "Bone Brew"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let _ = target.expect("AssertValidForTargetedPotion");
        // OstyCmd.Summon(ctx, target.Player, Summon.BaseValue, this).
        eng.summon_osty(Decimal::from(SUMMON))
    }
}
