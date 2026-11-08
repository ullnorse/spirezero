//! `Survivor` (`Models.Cards/Survivor.cs`): cost 1, 8 block (11 upgraded), then discard a card
//! from the hand.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, Prompt, PromptText, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Survivor;

#[sts2_macros::hooks]
impl Hooks for Survivor {
    type Me = CardId;
}

impl CardModel for Survivor {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Survivor"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn gains_block(&self) -> bool {
        true
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Basic
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Block, 8).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        // FromHandForDiscard(.., CardSelectorPrefs(prompt, 1), null): exactly one card, if any.
        let options = eng.s.player.hand.to_vec();
        let cards =
            eng.choose_cards(Prompt::card(play.card, PromptText::Discard), options, 1, 1)?;
        eng.discard(&cards)
    }
}
