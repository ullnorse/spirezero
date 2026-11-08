//! `CalculatedGamble` (`Models.Cards/CalculatedGamble.cs`): cost 0, Exhaust (+ Retain upgraded);
//! discards the whole hand and draws as many cards as the hand held.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType};

pub struct CalculatedGamble;

#[sts2_macros::hooks]
impl Hooks for CalculatedGamble {
    type Me = CardId;
}

impl CardModel for CalculatedGamble {
    fn name(&self) -> &'static str {
        "Calculated Gamble"
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
        card.keywords.insert(CardKeyword::Retain);
    }

    fn on_play(&self, eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        let cards = eng.s.player.hand.to_vec();
        let draw = i32::try_from(cards.len()).expect("the hand is small");
        eng.discard_and_draw(&cards, draw)
    }
}
