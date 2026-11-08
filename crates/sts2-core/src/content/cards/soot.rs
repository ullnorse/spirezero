//! `Soot` (`Models.Cards/Soot.cs`): status, unplayable (cost -1), no target, never upgraded.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType};

/// Status card (`BiiigHug`).
pub struct Soot;

#[sts2_macros::hooks]
impl Hooks for Soot {
    type Me = CardId;
}

impl CardModel for Soot {
    fn name(&self) -> &'static str {
        "Soot"
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
        Keywords::NONE.with(CardKeyword::Unplayable)
    }

    fn max_upgrade_level(&self) -> u8 {
        0
    }

    fn on_play(&self, _eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        Ok(())
    }
}
