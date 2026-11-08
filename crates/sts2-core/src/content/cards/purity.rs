//! `Purity` (`Models.Cards/Purity.cs`): cost 0, Retain, Exhaust; exhaust up to 3 (5 upgraded)
//! cards of the hand.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, Prompt, PromptText};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct Purity;

#[sts2_macros::hooks]
impl Hooks for Purity {
    type Me = CardId;
}

impl CardModel for Purity {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Purity"
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
        Keywords::NONE
            .with(CardKeyword::Retain)
            .with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 3).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let cards = eng.s.card(play.card).count_var_len(Var::Cards);
        // CardSelectCmd.FromHand(CardSelectorPrefs(prompt, 0, Cards)).
        let options = eng.s.player.hand.to_vec();
        for card in eng.choose_cards(Prompt::card(play.card, PromptText::Own), options, 0, cards)? {
            eng.exhaust(card, false)?;
        }
        Ok(())
    }
}
