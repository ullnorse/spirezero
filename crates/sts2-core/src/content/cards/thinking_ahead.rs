//! `ThinkingAhead` (`Models.Cards/ThinkingAhead.cs`): cost 0, Exhaust (upgrade removes it); draw 2
//! cards, then put a card of the hand on top of the draw pile.

use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType, Prompt, PromptText};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct ThinkingAhead;

#[sts2_macros::hooks]
impl Hooks for ThinkingAhead {
    type Me = CardId;
}

impl CardModel for ThinkingAhead {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Thinking Ahead"
    }

    fn cost(&self) -> i32 {
        0
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        eng.draw(eng.s.card(play.card).var(Var::Cards), false)?;
        // CardSelectCmd.FromHand(CardSelectorPrefs(prompt, 1)).
        let options = eng.s.player.hand.to_vec();
        if let Some(&card) = eng
            .choose_cards(Prompt::card(play.card, PromptText::Own), options, 1, 1)?
            .first()
        {
            eng.add_to_pile(card, PileType::Draw, Position::Top)?;
        }
        Ok(())
    }
}
