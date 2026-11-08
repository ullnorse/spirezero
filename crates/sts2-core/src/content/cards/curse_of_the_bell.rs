//! `CurseOfTheBell` (`Models.Cards/CurseOfTheBell.cs`): curse, Eternal and Unplayable; no effect.
//! `CallingBell` adds it to the deck.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType};

pub struct CurseOfTheBell;

#[sts2_macros::hooks]
impl Hooks for CurseOfTheBell {
    type Me = CardId;
}

impl CardModel for CurseOfTheBell {
    fn name(&self) -> &'static str {
        "Curse of the Bell"
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
            .with(CardKeyword::Eternal)
            .with(CardKeyword::Unplayable)
    }

    fn max_upgrade_level(&self) -> u8 {
        0
    }

    fn on_play(&self, _eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        Ok(())
    }
}
