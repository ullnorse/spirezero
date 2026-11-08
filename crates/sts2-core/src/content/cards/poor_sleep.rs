//! `PoorSleep` (`Models.Cards/PoorSleep.cs`): curse, unplayable and Retain (stays in hand); no effect.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType};

pub struct PoorSleep;

#[sts2_macros::hooks]
impl Hooks for PoorSleep {
    type Me = CardId;
}

impl CardModel for PoorSleep {
    fn name(&self) -> &'static str {
        "Poor Sleep"
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
            .with(CardKeyword::Retain)
    }

    fn max_upgrade_level(&self) -> u8 {
        0
    }

    fn on_play(&self, _eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        Ok(())
    }
}
