//! `Zap` (`Models.Cards/Zap.cs`): cost 1 (0 upgraded); channel 1 Lightning.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow, OrbKind};
use crate::content::cards::{CardModel, CardRarity, CardType, TargetType};

pub struct Zap;

#[sts2_macros::hooks]
impl Hooks for Zap {
    type Me = CardId;
}

impl CardModel for Zap {
    fn name(&self) -> &'static str {
        "Zap"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Basic
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn on_play(&self, eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        eng.channel(OrbKind::Lightning)
    }
}
