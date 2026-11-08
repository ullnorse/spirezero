//! `CunningPotion` (`Models.Potions/CunningPotion.cs`): the target player creates 3 upgraded
//! Shivs in the hand.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::{TargetType, shiv};
use crate::content::potions::{PotionModel, PotionUsage};

/// `CardsVar(3)`.
const CARDS: usize = 3;

pub struct CunningPotion;

#[sts2_macros::hooks]
impl Hooks for CunningPotion {
    type Me = PotionId;
}

impl PotionModel for CunningPotion {
    fn name(&self) -> &'static str {
        "Cunning Potion"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let _target = target.expect("AssertValidForTargetedPotion");
        for card in shiv::create_in_hand(eng, CARDS)? {
            eng.upgrade_card(card);
        }
        Ok(())
    }
}
