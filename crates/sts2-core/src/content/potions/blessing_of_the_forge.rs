//! `BlessingOfTheForge` (`Models.Potions/BlessingOfTheForge.cs`): upgrade every upgradable card in the target player's hand.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};

pub struct BlessingOfTheForge;

#[sts2_macros::hooks]
impl Hooks for BlessingOfTheForge {
    type Me = PotionId;
}

impl PotionModel for BlessingOfTheForge {
    fn name(&self) -> &'static str {
        "Blessing of the Forge"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let _ = target.expect("AssertValidForTargetedPotion");
        for card in eng.s.player.hand.clone() {
            if eng.s.card(card).is_upgradable() {
                eng.upgrade_card(card);
            }
        }
        Ok(())
    }
}
