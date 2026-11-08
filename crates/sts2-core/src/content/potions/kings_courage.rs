//! `KingsCourage` (`Models.Potions/KingsCourage.cs`): the target player Forges 15.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::decimal::Decimal;

/// `ForgeVar(15)`.
const FORGE: i32 = 15;

pub struct KingsCourage;

#[sts2_macros::hooks]
impl Hooks for KingsCourage {
    type Me = PotionId;
}

impl PotionModel for KingsCourage {
    fn name(&self) -> &'static str {
        "King's Courage"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let _ = target.expect("AssertValidForTargetedPotion");
        // The potion is the `source` of `AfterForge`, which only `HammerTimePower` reads.
        eng.forge(Decimal::from(FORGE), None)?;
        Ok(())
    }
}
