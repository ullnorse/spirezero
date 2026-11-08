//! `Chill` (`Models.Cards/Chill.cs`): cost 0, Exhaust (upgrade removes it); channel 1 Frost per hittable enemy.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow, OrbKind};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType};

pub struct Chill;

#[sts2_macros::hooks]
impl Hooks for Chill {
    type Me = CardId;
}

impl CardModel for Chill {
    fn name(&self) -> &'static str {
        "Chill"
    }

    fn cost(&self) -> i32 {
        0
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
        // One Frost per hittable enemy, counted once before the first channel.
        for _ in 0..eng.hooked().hittable_enemies().len() {
            eng.channel(OrbKind::Frost)?;
        }
        Ok(())
    }
}
