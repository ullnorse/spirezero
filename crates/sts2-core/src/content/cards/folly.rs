//! `Folly` (`Models.Cards/Folly.cs`): curse (`PreservedFog`), Unplayable, Eternal, Innate and
//! Ethereal; no effect.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType};

pub struct Folly;

#[sts2_macros::hooks]
impl Hooks for Folly {
    type Me = CardId;
}

impl CardModel for Folly {
    fn name(&self) -> &'static str {
        "Folly"
    }

    fn cost(&self) -> i32 {
        -1
    }

    fn card_type(&self) -> CardType {
        CardType::Curse
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Curse
    }

    fn target(&self) -> TargetType {
        TargetType::None
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE
            .with(CardKeyword::Unplayable)
            .with(CardKeyword::Eternal)
            .with(CardKeyword::Innate)
            .with(CardKeyword::Ethereal)
    }

    fn max_upgrade_level(&self) -> u8 {
        0
    }

    fn on_play(&self, _eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        Ok(())
    }
}
