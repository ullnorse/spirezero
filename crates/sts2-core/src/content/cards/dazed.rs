//! `Dazed` (`Models.Cards/Dazed.cs`): status, unplayable (cost -1), Ethereal, no target.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType};

/// Status card added by `EyeWithTeeth`.
pub struct Dazed;

#[sts2_macros::hooks]
impl Hooks for Dazed {
    type Me = CardId;
}

impl CardModel for Dazed {
    fn name(&self) -> &'static str {
        "Dazed"
    }

    fn cost(&self) -> i32 {
        -1
    }

    fn card_type(&self) -> CardType {
        CardType::Status
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Status
    }

    fn target(&self) -> TargetType {
        TargetType::None
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE
            .with(CardKeyword::Ethereal)
            .with(CardKeyword::Unplayable)
    }

    fn max_upgrade_level(&self) -> u8 {
        0
    }

    fn on_play(&self, _eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        Ok(())
    }
}
