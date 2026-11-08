//! Test-only card exercising mid-effect choices: choose a hand card and discard it, twice,
//! then draw one card.

#![cfg(test)]

use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType, Prompt, PromptText};
use crate::content::cards::{CardModel, CardRarity, CardType, TargetType};
use crate::decimal::Decimal;

/// Test-only card that asks for two hand selections.
pub struct TestChoice;

#[sts2_macros::hooks]
impl Hooks for TestChoice {
    type Me = CardId;
}

impl CardModel for TestChoice {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Test Choice"
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

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        on_play(eng, play.card)
    }
}

fn on_play(eng: &mut Engine<'_>, me: CardId) -> Flow {
    for _ in 0..2 {
        let options = eng.s.player.hand.to_vec();
        for card in eng.choose_cards(Prompt::card(me, PromptText::Discard), options, 1, 1)? {
            eng.add_to_pile(card, PileType::Discard, Position::Bottom)?;
        }
    }
    eng.draw(Decimal::ONE, false)?;
    Ok(())
}
