//! `LanternKey` (`Models.Cards/LanternKey.cs`): quest card, unplayable. Everything it does is
//! run-level: in Glory (`CurrentActIndex` 2) every `?` is an event and the next event is
//! `WarHistorianRepy` (`run/hooks.rs`), so it does nothing in combat.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType};

pub struct LanternKey;

#[sts2_macros::hooks]
impl Hooks for LanternKey {
    type Me = CardId;
}

impl CardModel for LanternKey {
    fn name(&self) -> &'static str {
        "Lantern Key"
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
        TargetType::Myself
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
