//! `CosmicConcoction` (`Models.Potions/CosmicConcoction.cs`): create 3 distinct random Colorless
//! cards, upgrade them and add them to the hand, one at a time.
//!
//! Generation stops the combat if it picks a Colorless card that is not ported.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PileType, PotionId};
use crate::content::cards::{TargetType, pools};
use crate::content::potions::{PotionModel, PotionUsage};

/// `CardsVar(3)`.
const CARDS: usize = 3;

pub struct CosmicConcoction;

#[sts2_macros::hooks]
impl Hooks for CosmicConcoction {
    type Me = PotionId;
}

impl PotionModel for CosmicConcoction {
    fn may_prompt(&self) -> bool {
        // Generated Colorless cards prompt (Discovery, ...).
        true
    }

    fn name(&self) -> &'static str {
        "Cosmic Concoction"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let player = target.expect("AssertValidForTargetedPotion");
        for card in eng.get_distinct_for_combat(&pools::colorless(), CARDS)? {
            eng.upgrade_card(card);
            eng.add_generated_card_to_combat(card, PileType::Hand, Some(player))?;
        }
        Ok(())
    }
}
