//! `BlockPotion` (`Models.Potions/BlockPotion.cs`): the target player gains 12 unpowered block.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId, ValueProps};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::decimal::Decimal;

/// `BlockVar(12m, ValueProp.Unpowered)`.
const BLOCK: i32 = 12;

pub struct BlockPotion;

#[sts2_macros::hooks]
impl Hooks for BlockPotion {
    type Me = PotionId;
}

impl PotionModel for BlockPotion {
    fn name(&self) -> &'static str {
        "Block Potion"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let target = target.expect("AssertValidForTargetedPotion");
        eng.gain_block(target, Decimal::from(BLOCK), ValueProps::UNPOWERED, None)?;
        Ok(())
    }
}
