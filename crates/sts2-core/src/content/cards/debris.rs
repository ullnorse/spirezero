//! `Debris` (`Models.Cards/Debris.cs`): status, cost 1, no target, Exhaust, not upgradable; does
//! nothing when played. Made by `CollisionCourse` and `CrashLanding`.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType};

pub struct Debris;

#[sts2_macros::hooks]
impl Hooks for Debris {
    type Me = CardId;
}

impl CardModel for Debris {
    fn name(&self) -> &'static str {
        "Debris"
    }

    fn cost(&self) -> i32 {
        1
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
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn max_upgrade_level(&self) -> u8 {
        0
    }

    fn on_play(&self, _eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        Ok(())
    }
}
