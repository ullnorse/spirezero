//! `Fusion` (`Models.Cards/Fusion.cs`): cost 1, Exhaust (upgrade removes it); channel 1 Plasma.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow, OrbKind};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType};

pub struct Fusion;

#[sts2_macros::hooks]
impl Hooks for Fusion {
    type Me = CardId;
}

impl CardModel for Fusion {
    fn name(&self) -> &'static str {
        "Fusion"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
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

    fn on_upgrade(&self, card: &mut Card) {
        card.keywords.remove(CardKeyword::Exhaust);
    }

    fn on_play(&self, eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        eng.channel(OrbKind::Plasma)
    }
}
