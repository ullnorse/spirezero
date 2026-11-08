//! `SkillPotion` (`Models.Potions/SkillPotion.cs`): choose 1 of 3 random Skill cards of the character's pool (or
//! none); it joins the hand and is free this turn.
//!
//! Generation panics if it picks a Necrobinder card that is not ported.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PileType, PotionId, PromptSource};
use crate::content::cards::{CardType, TargetType, pools};
use crate::content::potions::{PotionKind, PotionModel, PotionUsage};

/// The cards offered.
const CARDS: usize = 3;

pub struct SkillPotion;

#[sts2_macros::hooks]
impl Hooks for SkillPotion {
    type Me = PotionId;
}

impl PotionModel for SkillPotion {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Skill Potion"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let _ = target.expect("AssertValidForTargetedPotion");
        // The character pool's unlocked Skill cards.
        let pool: Vec<_> = pools::character(eng.s.player.character)
            .into_iter()
            .filter(|c| c.card_type == CardType::Skill)
            .collect();
        let options = eng.get_distinct_for_combat(&pool, CARDS)?;
        if let Some(card) =
            eng.choose_a_card(PromptSource::Potion(PotionKind::SkillPotion), options)?
        {
            // SetToFreeThisTurn.
            eng.s.set_to_free_this_turn(card);
            let owner = eng.s.player.creature;
            eng.add_generated_card_to_combat(card, PileType::Hand, Some(owner))?;
        }
        Ok(())
    }
}
