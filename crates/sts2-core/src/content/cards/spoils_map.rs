//! `SpoilsMap` (`Models.Cards/SpoilsMap.cs`): quest card, unplayable. Everything it does is run-level (the Act 2 spoils\n//! map, the quest gold `OnQuestComplete`), so it does nothing in combat.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct SpoilsMap;

#[sts2_macros::hooks]
impl Hooks for SpoilsMap {
    type Me = CardId;
}

impl CardModel for SpoilsMap {
    fn name(&self) -> &'static str {
        "Spoils Map"
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Gold, 600)];
        VARS
    }

    fn on_play(&self, _eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        Ok(())
    }
}
