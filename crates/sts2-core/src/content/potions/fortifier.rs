//! `Fortifier` (`Models.Potions/Fortifier.cs`): the target player doubles their Block.

use crate::combat::ValueProps;
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::decimal::Decimal;

pub struct Fortifier;

#[sts2_macros::hooks]
impl Hooks for Fortifier {
    type Me = PotionId;
}

impl PotionModel for Fortifier {
    fn name(&self) -> &'static str {
        "Fortifier"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let target = target.expect("AssertValidForTargetedPotion");
        let block = eng.s.creature(target).block;
        // CreatureCmd.GainBlock(target, target.Block * 2, ValueProp.Unpowered, null).
        eng.gain_block(
            target,
            Decimal::from(block * 2),
            ValueProps::UNPOWERED,
            None,
        )?;
        Ok(())
    }
}
