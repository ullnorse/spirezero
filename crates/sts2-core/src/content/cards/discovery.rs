//! `Discovery` (`Models.Cards/Discovery.cs`): cost 1, Exhaust (upgrading removes it); choose 1 of
//! 3 random cards of the character's pool (or none) to add to the hand, free this turn.
//!
//! Generation stops the combat (`Unsupported`) if it picks a Necrobinder card that is not ported.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType, PromptSource};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType, pools,
};

pub struct Discovery;

#[sts2_macros::hooks]
impl Hooks for Discovery {
    type Me = CardId;
}

impl CardModel for Discovery {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Discovery"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.keywords.remove(CardKeyword::Exhaust);
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let options = eng.get_distinct_for_combat(&pools::character(eng.s.player.character), 3)?;
        if let Some(card) = eng.choose_a_card(PromptSource::Card(play.card), options)? {
            // SetToFreeThisTurn.
            eng.s.set_to_free_this_turn(card);
            let player = eng.s.player.creature;
            eng.add_generated_card_to_combat(card, PileType::Hand, Some(player))?;
        }
        Ok(())
    }
}
