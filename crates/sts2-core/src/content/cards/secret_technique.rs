//! `SecretTechnique` (`Models.Cards/SecretTechnique.cs`): cost 0, Exhaust (upgrade removes it); choose a Skill from
//! the draw pile and put it in the hand.

use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType, Prompt, PromptText};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType};

pub struct SecretTechnique;

#[sts2_macros::hooks]
impl Hooks for SecretTechnique {
    type Me = CardId;
}

impl CardModel for SecretTechnique {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Secret Technique"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
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
        // CardSelectCmd.FromCombatPile(Draw, CardSelectorPrefs(prompt, 1), c => c.Type == Skill).
        let options: Vec<CardId> = eng
            .s
            .player
            .draw
            .iter()
            .copied()
            .filter(|&c| eng.s.card(c).card_type() == CardType::Skill)
            .collect();
        if let Some(&card) = eng
            .choose_cards(Prompt::card(play.card, PromptText::Own), options, 1, 1)?
            .first()
        {
            eng.add_to_pile(card, PileType::Hand, Position::Bottom)?;
        }
        Ok(())
    }
}
