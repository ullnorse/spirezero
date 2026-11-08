//! `OrobicAcid` (`Models.Potions/OrobicAcid.cs`): one random Attack, one Skill and one Power
//! card of the character's pool (three separate picks) join the hand, free this turn.
//!
//! Generation panics if it picks a Necrobinder card that is not ported.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PileType, PotionId};
use crate::content::cards::{CardType, TargetType, pools};
use crate::content::potions::{PotionModel, PotionUsage};

pub struct OrobicAcid;

#[sts2_macros::hooks]
impl Hooks for OrobicAcid {
    type Me = PotionId;
}

impl PotionModel for OrobicAcid {
    fn name(&self) -> &'static str {
        "Orobic Acid"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let _ = target.expect("AssertValidForTargetedPotion");
        let mut cards = Vec::new();
        for card_type in [CardType::Attack, CardType::Skill, CardType::Power] {
            let pool: Vec<_> = pools::character(eng.s.player.character)
                .into_iter()
                .filter(|c| c.card_type == card_type)
                .collect();
            cards.extend(eng.get_distinct_for_combat(&pool, 1)?);
        }
        for &card in &cards {
            // SetToFreeThisTurn.
            eng.s.set_to_free_this_turn(card);
        }
        // CardPileCmd.AddGeneratedCardsToCombat: card by card.
        let owner = eng.s.player.creature;
        for card in cards {
            eng.add_generated_card_to_combat(card, PileType::Hand, Some(owner))?;
        }
        Ok(())
    }
}
