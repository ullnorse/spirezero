//! Test-only relic exercising combat card generation and the choose-a-card screen, shaped like
//! `Toolbox`: before the first hand draw, 3 distinct generated cards from the ported uncommon
//! Necrobinder attacks; the chosen one joins the hand.

#![cfg(test)]

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, PileType, PromptSource, RelicId};
use crate::content::cards::pools;
use crate::content::cards::{CardRarity, CardType};
use crate::content::relics::RelicModel;

/// Test-only: generation and the choose-a-card screen.
pub struct TestChooseACard;

impl RelicModel for TestChooseACard {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Test Choose A Card"
    }
}

#[sts2_macros::hooks]
impl Hooks for TestChooseACard {
    type Me = RelicId;

    fn before_hand_draw(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        if eng.s.player.turn_number != 1 {
            return Ok(());
        }
        let options: Vec<_> = pools::character(eng.s.player.character)
            .into_iter()
            .filter(|c| {
                c.kind().is_some()
                    && c.card_type == CardType::Attack
                    && c.rarity == CardRarity::Uncommon
            })
            .collect();
        let cards = eng.get_distinct_for_combat(&options, 3)?;
        if let Some(card) = eng.choose_a_card(PromptSource::Relic(me), cards)? {
            eng.add_generated_card_to_combat(card, PileType::Hand, None)?;
        }
        Ok(())
    }
}
