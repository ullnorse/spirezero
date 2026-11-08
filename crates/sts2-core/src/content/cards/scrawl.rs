//! `Scrawl` (`Models.Cards/Scrawl.cs`): cost 1, Exhaust (upgrade adds Retain); draw until the
//! hand is full.

use crate::combat::commands::MAX_HAND_SIZE;
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType};
use crate::decimal::Decimal;

pub struct Scrawl;

#[sts2_macros::hooks]
impl Hooks for Scrawl {
    type Me = CardId;
}

impl CardModel for Scrawl {
    fn name(&self) -> &'static str {
        "Scrawl"
    }

    fn cost(&self) -> i32 {
        1
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
        card.keywords.insert(CardKeyword::Retain);
    }

    fn on_play(&self, eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        let count = MAX_HAND_SIZE.saturating_sub(eng.s.player.hand.len());
        eng.draw(
            Decimal::from(i32::try_from(count).expect("a hand fits an i32")),
            false,
        )?;
        Ok(())
    }
}
