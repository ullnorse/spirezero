//! `Wish` (`Models.Cards/Wish.cs`): Ancient skill (`SereTalon`), cost 0, Exhaust; choose 1 card
//! of the draw pile and put it into the hand. The upgrade adds Retain.

use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType, Prompt, PromptText};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType};

pub struct Wish;

#[sts2_macros::hooks]
impl Hooks for Wish {
    type Me = CardId;
}

impl CardModel for Wish {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Wish"
    }

    fn cost(&self) -> i32 {
        0
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
        card.keywords.insert(CardKeyword::Retain);
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        // CardSelectCmd.FromCombatPile(Draw, CardSelectorPrefs(prompt, 1)).FirstOrDefault().
        let options = eng.s.player.draw.to_vec();
        if let Some(&card) = eng
            .choose_cards(Prompt::card(play.card, PromptText::Own), options, 1, 1)?
            .first()
        {
            eng.add_to_pile(card, PileType::Hand, Position::Bottom)?;
        }
        Ok(())
    }
}
