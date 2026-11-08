//! `ColorlessPotion` (`Models.Potions/ColorlessPotion.cs`): choose 1 of 3 random cards of the
//! Colorless pool (or none); it joins the hand and is free this turn.
//!
//! Generation panics if it picks a Colorless card that is not ported.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PileType, PotionId, PromptSource};
use crate::content::cards::{TargetType, pools};
use crate::content::potions::{PotionKind, PotionModel, PotionUsage};

/// The cards offered.
const CARDS: usize = 3;

pub struct ColorlessPotion;

#[sts2_macros::hooks]
impl Hooks for ColorlessPotion {
    type Me = PotionId;
}

impl PotionModel for ColorlessPotion {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Colorless Potion"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let _ = target.expect("AssertValidForTargetedPotion");
        let options = eng.get_distinct_for_combat(&pools::colorless(), CARDS)?;
        if let Some(card) =
            eng.choose_a_card(PromptSource::Potion(PotionKind::ColorlessPotion), options)?
        {
            // SetToFreeThisTurn.
            eng.s.set_to_free_this_turn(card);
            let owner = eng.s.player.creature;
            eng.add_generated_card_to_combat(card, PileType::Hand, Some(owner))?;
        }
        Ok(())
    }
}
