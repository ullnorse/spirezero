//! `ByrdonisEgg` (`Models.Cards/ByrdonisEgg.cs`): quest card, unplayable. Its only effect is run-level (`TryModifyRestSiteOptions` adds the
//! hatch rest-site option, see `run::hooks`), so it does nothing in combat.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType};

pub struct ByrdonisEgg;

#[sts2_macros::hooks]
impl Hooks for ByrdonisEgg {
    type Me = CardId;
}

impl CardModel for ByrdonisEgg {
    fn name(&self) -> &'static str {
        "Byrdonis Egg"
    }

    fn cost(&self) -> i32 {
        -1
    }

    fn card_type(&self) -> CardType {
        CardType::Quest
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Quest
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
