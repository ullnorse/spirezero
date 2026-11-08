//! `Prepared` (`Models.Cards/Prepared.cs`): cost 0, draw 1 card (2 upgraded), then discard as many.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, Prompt, PromptText};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Prepared;

#[sts2_macros::hooks]
impl Hooks for Prepared {
    type Me = CardId;
}

impl CardModel for Prepared {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Prepared"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 1).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let count = eng.s.card(play.card).count_var_len(Var::Cards);
        eng.draw(eng.s.card(play.card).var(Var::Cards), false)?;
        // FromHandForDiscard(prefs: cardCount): exactly `count` cards.
        let options = eng.s.player.hand.to_vec();
        let cards = eng.choose_cards(
            Prompt::card(play.card, PromptText::Discard),
            options,
            count,
            count,
        )?;
        eng.discard(&cards)
    }
}
