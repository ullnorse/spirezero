//! `Acrobatics` (`Models.Cards/Acrobatics.cs`): cost 1, draw 3 cards (4 upgraded), then discard 1.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, Prompt, PromptText};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct Acrobatics;

#[sts2_macros::hooks]
impl Hooks for Acrobatics {
    type Me = CardId;
}

impl CardModel for Acrobatics {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Acrobatics"
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 3).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let cards = eng.s.card(play.card).int_var(Var::Cards);
        eng.draw(Decimal::from(cards), false)?;
        // FromHandForDiscard(prefs: 1).FirstOrDefault(): exactly one card, if any.
        let options = eng.s.player.hand.to_vec();
        if let Some(&card) = eng
            .choose_cards(Prompt::card(play.card, PromptText::Discard), options, 1, 1)?
            .first()
        {
            eng.discard(&[card])?;
        }
        Ok(())
    }
}
