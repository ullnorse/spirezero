//! `Abundance` (`Models.Cards/Abundance.cs`): the Ancient reward of the `Dowsing` quest. Cost 1
//! (0 upgraded), Exhaust; choose 1 of 3 random upgraded Power cards of the character's pool to
//! add to the hand, free this turn. Cannot be generated in combat.
//!
//! Generation stops the combat (`Unsupported`) if it picks a Necrobinder Power that is not ported.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType, PromptSource};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType, pools,
};

pub struct Abundance;

#[sts2_macros::hooks]
impl Hooks for Abundance {
    type Me = CardId;
}

impl CardModel for Abundance {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Abundance"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Ancient
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let powers: Vec<_> = pools::character(eng.s.player.character)
            .into_iter()
            .filter(|c| c.card_type == CardType::Power)
            .collect();
        let options = eng.get_distinct_for_combat(&powers, 3)?;
        for &card in &options {
            eng.upgrade_card(card);
        }
        if let Some(card) = eng.choose_a_card(PromptSource::Card(play.card), options)? {
            // SetToFreeThisTurn.
            eng.s.set_to_free_this_turn(card);
            let player = eng.s.player.creature;
            eng.add_generated_card_to_combat(card, PileType::Hand, Some(player))?;
        }
        Ok(())
    }
}
