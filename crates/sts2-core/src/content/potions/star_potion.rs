//! `StarPotion` (`Models.Potions/StarPotion.cs`): the target player gains 3 stars.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::decimal::Decimal;

/// `StarsVar(3)`.
const STARS: i32 = 3;

pub struct StarPotion;

#[sts2_macros::hooks]
impl Hooks for StarPotion {
    type Me = PotionId;
}

impl PotionModel for StarPotion {
    fn name(&self) -> &'static str {
        "Star Potion"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let _ = target.expect("AssertValidForTargetedPotion");
        // PlayerCmd.GainStars(Stars.BaseValue, target.Player): the only player.
        eng.gain_stars(Decimal::from(STARS))
    }
}
